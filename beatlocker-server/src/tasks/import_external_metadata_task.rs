use crate::db::{DbAlbum, DbArtist, DbSong};
use crate::tasks::{await_join_set, insert_cover_art};
use crate::{
    discogs_client, get_cover_art_archive, get_discogs, get_musicbrainz, wrap_err, AppResult,
    CoverArtArchiveImagesResponse, DiscogsMasterResponse, DiscogsResourceResponse,
    DiscogsSearchResponse, DiscogsSearchResult, MusicbrainzArtist, MusicbrainzArtistsResponse,
    MusicbrainzRecording, MusicbrainzRecordingsResponse, TaskState,
};
use anyhow::anyhow;
use heck::ToTitleCase;
use reqwest::Method;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use std::fmt::Debug;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;
use tracing::{debug, info};
use unidecode::unidecode;
use uuid::Uuid;

// Process the metadata of X songs at a time, to make sure we don't spawn a ton of tasks that are
// all just waiting for rate limiters
const CHUNK_SIZE: usize = 8;

pub async fn import_external_metadata(state: Arc<TaskState>) -> AppResult<()> {
    if !state.options.import_external_metadata {
        return Ok(());
    }

    let mut conn = state.db.conn().await?;

    // Only update metadata if this hasn't already happened X hours ago
    let timestamp = chrono::offset::Utc::now() - chrono::Duration::hours(96);

    // Grab all songs that may require updating and have not recently been touched
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
            let discogs_token = state.options.discogs_token.clone();
            set.spawn(async move {
                let path = info.path.as_os_str().to_string_lossy();
                debug!(?path, "Updating metadata");

                let ctx = UpdateContext {
                    state: &state,
                    discogs_token: discogs_token.as_ref(),
                    info: &info,
                };

                state.db.update_last_updated(info.folder_child_id).await?;
                wrap_err(
                    update_artist(&ctx, get_db_song_info(&state, &info).await?),
                    || (),
                )
                .await;
                wrap_err(
                    update_genre(&ctx, get_db_song_info(&state, &info).await?),
                    || (),
                )
                .await;
                wrap_err(
                    update_song_cover_art(&ctx, get_db_song_info(&state, &info).await?),
                    || (),
                )
                .await;
                wrap_err(
                    update_album_cover_art(&ctx, get_db_song_info(&state, &info).await?),
                    || (),
                )
                .await;
                wrap_err(
                    update_artist_cover_art(&ctx, get_db_song_info(&state, &info).await?),
                    || (),
                )
                .await;

                info!(?path, "Completed updating metadata");

                Ok(())
            });
        }

        await_join_set(set).await?;
    }

    Ok(())
}

async fn update_artist(ctx: &UpdateContext<'_>, db: DbSongInfo) -> AppResult<()> {
    if db.artist.musicbrainz_id.is_some() {
        return Ok(());
    }

    if let Some(mut mb_song) = musicbrainz_find_song(ctx.info).await? {
        if let Some(mb_arid) = mb_song.artist_credit.pop().map(|c| c.artist.id) {
            debug!(ctx.info.artist_name, mb_arid, "Updating artist information");
            sqlx::query("UPDATE artists SET musicbrainz_id = ? WHERE artist_id = ?")
                .bind(mb_arid)
                .bind(ctx.info.artist_id)
                .execute(ctx.state.db.conn().await?.deref_mut())
                .await?;
        }
    }

    Ok(())
}

async fn update_genre(ctx: &UpdateContext<'_>, db: DbSongInfo) -> AppResult<()> {
    if db.song.genre.is_some() {
        return Ok(());
    }

    let mut genre = None;
    if let Some(mut mb_song) = musicbrainz_find_song(ctx.info).await? {
        genre = mb_song.tags.pop().map(|t| t.name);
        if genre.is_none() {
            if let Some(artist_id) = mb_song.artist_credit.pop().map(|c| c.artist.id) {
                if let Some(mut artist) = musicbrainz_find_artist(artist_id).await? {
                    genre = artist.tags.pop().map(|t| t.name);
                }
            }
        };
    }

    if genre.is_none() {
        if let Some(mut discogs) = discogs_find_song(ctx).await? {
            genre = discogs.genre.pop();
        }
    }

    if let Some(genre) = genre {
        let genre = genre.to_title_case();
        debug!(ctx.info.song_title, genre, "Updating genre information");
        sqlx::query("UPDATE songs SET genre = ? WHERE song_id = ?")
            .bind(genre)
            .bind(ctx.info.song_id)
            .execute(ctx.state.db.conn().await?.deref_mut())
            .await?;
    }

    Ok(())
}

async fn update_song_cover_art(ctx: &UpdateContext<'_>, db: DbSongInfo) -> AppResult<()> {
    if db.song.cover_art_id.is_some() {
        return Ok(());
    }

    let mut url = None;
    if let Some(mut mb_song) = musicbrainz_find_song(ctx.info).await? {
        if let Some(release) = mb_song.releases.pop() {
            let images: Option<CoverArtArchiveImagesResponse> =
                get_cover_art_archive("release", &release.id).await?;
            if let Some(mut images) = images {
                url = images.images.pop().and_then(|i| i.image);
            }
        }
    }

    if url.is_none() {
        if let Some(discogs) = discogs_find_song(ctx).await? {
            url = discogs.cover_image.or(discogs.thumb);
        }
    }

    if let Some(url) = url {
        if !url.is_empty() {
            debug!(url, ctx.info.song_title, "Updating song cover art");
            let cover_art_id = insert_cover_art(&ctx.state.db, &url).await?;
            sqlx::query("UPDATE songs SET cover_art_id = ? WHERE song_id = ?")
                .bind(cover_art_id)
                .bind(db.song.song_id)
                .execute(ctx.state.db.conn().await?.deref_mut())
                .await?;
        }
    }

    Ok(())
}

async fn update_album_cover_art(ctx: &UpdateContext<'_>, db: DbSongInfo) -> AppResult<()> {
    if let Some(db_album) = &db.album {
        if db_album.cover_art_id.is_some() {
            return Ok(());
        }

        let mut url = None;
        if let Some(discogs_token) = ctx.discogs_token {
            if let Some(discogs) = discogs_find_song(ctx).await? {
                if let Some(master_url) = &discogs.master_url {
                    if !master_url.is_empty() {
                        debug!(master_url, "Getting Discogs master");
                        let response = discogs_client()
                            .request(Method::GET, master_url)
                            .query(&[("token", &discogs_token)])
                            .send()
                            .await?;
                        let mut master = response.json::<DiscogsMasterResponse>().await?;
                        url = master.images.pop().and_then(|u| u.resource_url);
                    }
                }
            }
        }

        if let Some(url) = url {
            if !url.is_empty() {
                debug!(url, ctx.info.album_title, "Updating album cover art");
                let cover_art_id = insert_cover_art(&ctx.state.db, &url).await?;
                sqlx::query("UPDATE albums SET cover_art_id = ? WHERE album_id = ?")
                    .bind(cover_art_id)
                    .bind(db_album.album_id)
                    .execute(ctx.state.db.conn().await?.deref_mut())
                    .await?;
            }
        }
    }

    Ok(())
}

async fn update_artist_cover_art(ctx: &UpdateContext<'_>, db: DbSongInfo) -> AppResult<()> {
    if db.artist.cover_art_id.is_some() {
        return Ok(());
    }

    let mut url = None;
    if let Some(discogs_token) = ctx.discogs_token {
        if let Some(discogs) = discogs_find_song(ctx).await? {
            if let Some(resource_url) = &discogs.resource_url {
                if !resource_url.is_empty() {
                    debug!(resource_url, "Getting Discogs resource");
                    let response = discogs_client()
                        .request(Method::GET, resource_url)
                        .query(&[("token", &discogs_token)])
                        .send()
                        .await?;
                    let mut resource = response.json::<DiscogsResourceResponse>().await?;

                    if let Some(artist) = resource.artists.pop() {
                        url = artist.thumbnail_url;
                    }
                }
            }
        }
    }

    if let Some(url) = url {
        if !url.is_empty() {
            debug!(url, ctx.info.artist_name, "Updating photo");
            let cover_art_id = insert_cover_art(&ctx.state.db, &url).await?;
            sqlx::query("UPDATE artists SET cover_art_id = ? WHERE artist_id = ?")
                .bind(cover_art_id)
                .bind(ctx.info.artist_id)
                .execute(ctx.state.db.conn().await?.deref_mut())
                .await?;
        }
    }

    Ok(())
}

async fn get_db_song_info(state: &TaskState, info: &SongInfo) -> AppResult<DbSongInfo> {
    let song = state.db.find_song_by_id(info.song_id).await?;
    let song = match song {
        Some(song) => song,
        None => return Err(anyhow!("Song not found").into()),
    };

    let artist = state.db.find_artist_by_id(info.artist_id).await?;
    let artist = match artist {
        Some(artist) => artist,
        None => return Err(anyhow!("Artist not found").into()),
    };

    let album = match info.album_id {
        Some(id) => Some(state.db.find_album_by_id(id).await?),
        None => None,
    }
    .flatten();

    Ok(DbSongInfo {
        song,
        artist,
        album,
    })
}

async fn musicbrainz_find_song(info: &SongInfo) -> AppResult<Option<MusicbrainzRecording>> {
    let mut query = format!(
        "query=title:{} AND artist:{}",
        info.song_title, info.artist_name
    );
    if let Some(album_title) = &info.album_title {
        query += &format!(" AND release:{}", album_title);
    }

    let query = &[("fmt", "json"), ("query", &query)];

    let response: Option<MusicbrainzRecordingsResponse> =
        get_musicbrainz("recording", &query).await?;
    Ok(response.and_then(|mut r| r.recordings.pop()))
}

async fn musicbrainz_find_artist(artist_id: String) -> AppResult<Option<MusicbrainzArtist>> {
    let query = &[("fmt", "json"), ("query", &format!("arid:{}", artist_id))];
    let artists_response: Option<MusicbrainzArtistsResponse> =
        get_musicbrainz("artist", &query).await?;
    Ok(artists_response.and_then(|mut r| r.artists.pop()))
}

async fn discogs_find_song(ctx: &UpdateContext<'_>) -> AppResult<Option<DiscogsSearchResult>> {
    if let Some(discogs_token) = ctx.discogs_token {
        let query = &[
            ("artist", &unidecode(&ctx.info.artist_name)),
            (
                "release_title",
                &ctx.info
                    .album_title
                    .as_ref()
                    .map(|t| unidecode(t))
                    .unwrap_or_default(),
            ),
            ("track", &unidecode(&ctx.info.song_title)),
            ("token", discogs_token),
        ];

        let search_response: Option<DiscogsSearchResponse> = get_discogs("search", query).await?;
        match search_response.and_then(|mut r| r.results.pop()) {
            Some(response) => Ok(Some(response)),
            None => {
                // Try again without the album title
                let query = &[
                    ("artist", &unidecode(&ctx.info.artist_name)),
                    ("track", &unidecode(&ctx.info.song_title)),
                    ("token", discogs_token),
                ];
                let search_response: Option<DiscogsSearchResponse> =
                    get_discogs("search", query).await?;
                Ok(search_response.and_then(|mut r| r.results.pop()))
            }
        }
    } else {
        Ok(None)
    }
}

struct UpdateContext<'a> {
    state: &'a TaskState,
    discogs_token: Option<&'a String>,
    info: &'a SongInfo,
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

#[derive(Debug)]
struct DbSongInfo {
    song: DbSong,
    artist: DbArtist,
    album: Option<DbAlbum>,
}
