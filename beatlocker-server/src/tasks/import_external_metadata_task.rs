use crate::tasks::{await_join_set, insert_cover_art};
use crate::{
    reqwest_client_builder, wrap_err, AppResult, ExponentialBackoff, RateLimiterMiddleware,
    TaskState,
};
use governor::Quota;
use reqwest::header::CONTENT_TYPE;
use reqwest::Method;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::RetryTransientMiddleware;
use serde::Deserialize;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;
use tracing::{debug, error, info};
use uuid::Uuid;

// Process the metadata of X songs at a time, to make sure we don't spawn a ton of tasks that are
// all just waiting for rate limiters
const CHUNK_SIZE: usize = 8;

pub async fn import_external_metadata(state: Arc<TaskState>) -> AppResult<()> {
    if !state.options.import_external_metadata {
        return Ok(());
    }

    let discogs_token = match &state.options.discogs_token {
        Some(token) => token.clone(),
        None => return Ok(()),
    };

    let mut conn = state.db.conn().await?;

    // Only update metadata if this hasn't already happened X hours ago
    let timestamp = chrono::offset::Utc::now() - chrono::Duration::hours(24);

    let results = sqlx::query(
        r#"
            SELECT fc.path, fc.folder_child_id, songs.song_id as song_id, songs.title as song_title, albums.album_id as album_id, albums.title as album_title, artists.artist_id as artist_id, artists.name as artist_name
            FROM songs
            LEFT JOIN albums on albums.album_id = songs.album_id
            LEFT JOIN album_artists aa on albums.album_id = aa.album_id
            LEFT JOIN artists on songs.artist_id = artists.artist_id
            LEFT JOIN folder_children fc on songs.song_id = fc.song_id
            WHERE (songs.cover_art_id is null
            OR artists.cover_art_id is null
            OR albums.cover_art_id is null
            OR songs.genre is null)
            AND (fc.last_updated is null OR fc.last_updated < ?)
        "#,
    )
        .bind(timestamp)
    .map(|row: SqliteRow| {
        let path: String = row.get("path");
        SongInfo {
            path: PathBuf::from(path),
            folder_child_id: row.get("folder_child_id"),
            song_id: row.get("song_id"),
            song_title: row.get("song_title"),
            album_id: row.get("album_id"),
            album_title: row.get("album_title"),
            artist_id: row.get("artist_id"),
            artist_name: row.get("artist_name")
        }
    })
    .fetch(conn.deref_mut())
        .chunks_timeout(CHUNK_SIZE, Duration::from_secs(10));
    tokio::pin!(results);

    while let Some(chunk) = results.next().await {
        let mut set = JoinSet::new();
        for info in chunk.into_iter().flatten() {
            let state = state.clone();
            let discogs_token = discogs_token.clone();
            set.spawn(async move {
                let path = info.path.as_os_str().to_string_lossy();
                debug!(?path, "Updating metadata");

                state.db.update_last_updated(info.folder_child_id).await?;

                if !wrap_err(
                    update_discogs_metadata(&state, discogs_token.clone(), &info, true),
                    || true,
                )
                .await
                {
                    // try again, but without album info this time
                    let _ = wrap_err(
                        update_discogs_metadata(&state, discogs_token.clone(), &info, false),
                        || true,
                    )
                    .await;
                }

                info!(?path, "Completed updating metadata");

                Ok(())
            });
        }

        await_join_set(set).await?;
    }

    Ok(())
}

async fn update_discogs_metadata(
    state: &TaskState,
    discogs_token: String,
    info: &SongInfo,
    include_album_in_search: bool,
) -> AppResult<bool> {
    let safe_album_title = info
        .album_title
        .clone()
        .unwrap_or_default()
        .replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "");
    let safe_artist_name = info
        .artist_name
        .replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "");
    let safe_song_title = info
        .song_title
        .replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "");

    let query_release_title = if include_album_in_search {
        safe_album_title
    } else {
        "".to_string()
    };
    let query = &[
        ("artist", &safe_artist_name),
        ("release_title", &query_release_title),
        ("track", &safe_song_title),
        ("token", &discogs_token),
    ];
    debug!(?query, "Sending search query");
    let response = discogs_client()
        .request(Method::GET, "https://api.discogs.com/database/search")
        .header(CONTENT_TYPE, "application/json")
        .query(query)
        .send()
        .await?;

    let existing_song = state.db.find_song_by_id(info.song_id).await?;
    let existing_song = match existing_song {
        Some(song) => song,
        None => return Ok(true),
    };

    let existing_artist = state.db.find_artist_by_id(info.artist_id).await?;
    let existing_artist = match existing_artist {
        Some(artist) => artist,
        None => return Ok(true),
    };

    let existing_album = match info.album_id {
        Some(id) => Some(state.db.find_album_by_id(id).await?),
        None => None,
    }
    .flatten();

    let status_code = response.status();
    let json = response.text().await?;
    match serde_json::from_str::<DiscogsSearchResponse>(&json) {
        Ok(search_response) => {
            if let Some(result) = search_response.results.first() {
                // Update song cover art
                if existing_song.cover_art_id.is_none() {
                    if let Some(url) = result.cover_image.as_ref().or(result.thumb.as_ref()) {
                        if !url.is_empty() {
                            debug!(url, info.song_title, "Updating song cover art");
                            let cover_art_id = insert_cover_art(&state.db, url).await?;
                            sqlx::query("UPDATE songs SET cover_art_id = ? WHERE song_id = ?")
                                .bind(cover_art_id)
                                .bind(existing_song.song_id)
                                .execute(state.db.conn().await?.deref_mut())
                                .await?;
                        }
                    }
                }

                // Update song genre
                if existing_song.genre.is_none() {
                    if let Some(genre) = result.genre.first() {
                        debug!(info.song_title, "Updating genre information");
                        sqlx::query("UPDATE songs SET genre = ? WHERE song_id = ?")
                            .bind(genre)
                            .bind(info.song_id)
                            .execute(state.db.conn().await?.deref_mut())
                            .await?;
                    }
                }

                // Find album image
                if let Some(album) = &existing_album {
                    if album.cover_art_id.is_none() {
                        if let Some(master_url) = &result.master_url {
                            if !master_url.is_empty() {
                                debug!(master_url, "Getting Discogs master");
                                let response = discogs_client()
                                    .request(Method::GET, master_url)
                                    .query(&[("token", &discogs_token)])
                                    .send()
                                    .await?;
                                let master = response.json::<DiscogsMasterResponse>().await?;

                                if let Some(images) = master.images {
                                    if let Some(image) = images.first() {
                                        if let Some(url) = &image.resource_url {
                                            if !url.is_empty() {
                                                debug!(
                                                    url,
                                                    info.album_title, "Updating album cover art"
                                                );
                                                let cover_art_id =
                                                    insert_cover_art(&state.db, url).await?;
                                                sqlx::query(
                                                    "UPDATE albums SET cover_art_id = ? WHERE album_id = ?",
                                                )
                                                    .bind(cover_art_id)
                                                    .bind(album.album_id)
                                                    .execute(state.db.conn().await?.deref_mut())
                                                    .await?;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Find artist image
                if existing_artist.cover_art_id.is_none() {
                    if let Some(resource_url) = &result.resource_url {
                        if !resource_url.is_empty() {
                            debug!(resource_url, "Getting Discogs resource");
                            let response = discogs_client()
                                .request(Method::GET, resource_url)
                                .query(&[("token", &discogs_token)])
                                .send()
                                .await?;
                            let resource = response.json::<DiscogsResourceResponse>().await?;

                            if let Some(artists) = resource.artists {
                                if let Some(artist) = artists.first() {
                                    if let Some(url) = &artist.thumbnail_url {
                                        if !url.is_empty() {
                                            debug!(url, info.artist_name, "Updating photo");
                                            let cover_art_id =
                                                insert_cover_art(&state.db, url).await?;
                                            sqlx::query(
                                                "UPDATE artists SET cover_art_id = ? WHERE artist_id = ?",
                                            )
                                                .bind(cover_art_id)
                                                .bind(info.artist_id)
                                                .execute(state.db.conn().await?.deref_mut())
                                                .await?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(true)
            } else {
                Ok(false)
            }
        }
        Err(_) => {
            error!(
                ?status_code,
                ?json,
                "Problem decoding Discogs JSON response"
            );
            Ok(true)
        }
    }
}

#[derive(Debug)]
struct SongInfo {
    path: PathBuf,
    folder_child_id: Uuid,
    song_id: Uuid,
    song_title: String,
    album_id: Option<Uuid>,
    album_title: Option<String>,
    artist_id: Uuid,
    artist_name: String,
}

#[derive(Debug, Deserialize)]
struct DiscogsSearchResponse {
    results: Vec<DiscogsSearchResult>,
}

#[derive(Debug, Deserialize)]
struct DiscogsSearchResult {
    genre: Vec<String>,
    cover_image: Option<String>,
    thumb: Option<String>,
    master_url: Option<String>,
    resource_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscogsMasterResponse {
    images: Option<Vec<DiscogsImage>>,
}

#[derive(Debug, Deserialize)]
struct DiscogsResourceResponse {
    artists: Option<Vec<DiscogsArtist>>,
}

#[derive(Debug, Deserialize)]
struct DiscogsArtist {
    thumbnail_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscogsImage {
    resource_url: Option<String>,
}

static DISCOGS_CLIENT: once_cell::sync::OnceCell<ClientWithMiddleware> =
    once_cell::sync::OnceCell::new();

fn discogs_client() -> &'static ClientWithMiddleware {
    DISCOGS_CLIENT.get_or_init(|| {
        // Allow 1 request every 2 seconds, otherwise we'll get rate limited
        let quota = Quota::with_period(Duration::from_secs(2)).unwrap();

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(20), Duration::from_secs(300))
            .build_with_max_retries(3);
        reqwest_client_builder()
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .with(RateLimiterMiddleware::new(quota))
            .build()
    })
}
