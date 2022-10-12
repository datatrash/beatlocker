#![allow(dead_code, unused)]
use crate::db::{DbAlbum, DbArtist, DbCoverArt, DbFolder, DbFolderChild, DbSong};
use crate::tasks::extract_metadata::extract_metadata;
use crate::{reqwest_client, uri_to_uuid, AppResult, Db, ServerOptions};

use crate::tasks::providers::{
    FindCoverArtQuery, InfoProvider, InfoProviderList, InfoProviderOptions, ProviderUri, Release,
};
use async_recursion::async_recursion;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::types::Uuid;
use sqlx::Row;
use std::fmt::Debug;
use std::ops::DerefMut;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

mod extract_metadata;
mod providers;

#[derive(Debug)]
pub enum TaskMessage {
    ImportFolder(PathBuf, Option<Uuid>),
    ImportMissingAlbumCoverArt,
    ImportMissingArtistCoverArt,
    ImportMissingSongCoverArt,
    Shutdown,
}

pub enum TaskReply {
    ShutdownComplete,
}

pub struct TaskState {
    db: Arc<Db>,
    now_provider: Arc<Box<dyn Fn() -> DateTime<Utc> + Send + Sync>>,
    provider_list: Arc<InfoProviderList>,
    root_path: PathBuf,
}

pub async fn start_task_runner(
    db: Arc<Db>,
    options: &ServerOptions,
) -> AppResult<(Sender<TaskMessage>, Receiver<TaskReply>)> {
    let (tx, mut rx) = mpsc::channel(32);
    let (reply_tx, reply_rx) = mpsc::channel(32);

    let task_state = Arc::new(TaskState {
        db: db.clone(),
        now_provider: options.now_provider.clone(),
        provider_list: Arc::new(InfoProviderList::new(&InfoProviderOptions {
            discogs_token: options.discogs_token.clone(),
        })),
        root_path: options.path.clone(),
    });

    db.insert_folder_if_not_exists(&DbFolder {
        folder_id: Uuid::nil(),
        parent_id: None,
        uri: "root".to_owned(),
        name: "root".to_owned(),
        cover_art_id: None,
        created: (task_state.now_provider)(),
    })
    .await?;

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let result = match message {
                TaskMessage::ImportFolder(path, parent_folder_id) => {
                    import_folder(task_state.clone(), &path, parent_folder_id).await
                }
                TaskMessage::ImportMissingAlbumCoverArt => {
                    import_missing_album_cover_art(task_state.clone()).await
                }
                TaskMessage::ImportMissingArtistCoverArt => {
                    import_missing_artist_cover_art(task_state.clone()).await
                }
                TaskMessage::ImportMissingSongCoverArt => {
                    import_missing_song_cover_art(task_state.clone()).await
                }
                TaskMessage::Shutdown => {
                    info!("Shutting down background tasks...");
                    let _ = reply_tx.send(TaskReply::ShutdownComplete);
                    break;
                }
            };

            match result {
                Ok(_) => {}
                Err(e) => {
                    error!(?e, "Error in background task");
                }
            }
        }
    });

    Ok((tx, reply_rx))
}

#[async_recursion]
pub async fn import_folder(
    state: Arc<TaskState>,
    folder: &Path,
    parent_folder_id: Option<Uuid>,
) -> AppResult<()> {
    debug!(?folder, "Processing folder");

    // Insert folder in DB
    let folder_id = if folder == state.root_path {
        Uuid::nil()
    } else {
        let folder_name = folder.file_name().unwrap();
        let folder_uri = format!("path:{}", folder.to_str().unwrap());

        state
            .db
            .insert_folder_if_not_exists(&DbFolder {
                folder_id: uri_to_uuid(&folder_uri),
                parent_id: parent_folder_id,
                uri: folder_uri.clone(),
                name: folder_name.to_string_lossy().to_string(),
                cover_art_id: None,
                created: (state.now_provider)(),
            })
            .await?
    };

    let mut set = JoinSet::new();
    let mut read_dir = tokio::fs::read_dir(folder).await?;
    loop {
        let entry = read_dir.next_entry().await?;
        if let Some(entry) = entry {
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                let state = state.clone();
                let entry = entry.path().clone();
                let folder_id = folder_id.clone();
                set.spawn(
                    async move { import_folder(state, entry.as_path(), Some(folder_id)).await },
                );
            }
            if file_type.is_file()
                && entry
                    .path()
                    .extension()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    == Some("ogg".to_string())
            {
                let folder_id = folder_id.clone();
                let state = state.clone();
                let entry = entry.path().clone();
                set.spawn(async move { import_file(state, entry.as_path(), folder_id).await });
            }
        } else {
            break;
        }
    }

    await_join_set(set).await?;

    debug!(?folder, "Processing folder done");
    Ok(())
}

async fn import_file(state: Arc<TaskState>, path: &Path, folder_id: Uuid) -> AppResult<()> {
    let folder_child_path = path.to_str().unwrap().to_string();
    if state
        .db
        .find_folder_child_by_path(&folder_child_path)
        .await?
        .is_some()
    {
        debug!(?path, "Rejecting file due to path");
        return Ok(());
    }

    warn!(?path, "Importing file");
    let (metadata, file_size) = {
        let file = std::fs::File::open(path)?;
        (
            extract_metadata(path.file_name(), &file)?,
            file.metadata()?.len() as u32,
        )
    };
    if metadata.is_none() {
        warn!(?path, "Could not extract metadata");
        return Ok(());
    }
    let metadata = metadata.unwrap();

    let release = Release {
        album: metadata
            .album
            .map(|a| ((ProviderUri::from_provider("tags", &a), a.to_string()))),
        album_artist: None,
        artist: Some((
            ProviderUri::from_provider("tags", &metadata.artist),
            metadata.artist,
        )),
        song: ((
            ProviderUri::from_provider("tags", &metadata.title),
            metadata.title.to_string(),
        )),
        genre: None,
        release_date: None,
    };

    /*
    state
        .provider_list
        .find_release(&FindReleaseQuery {
            album: metadata.album.as_deref(),
            artist: &metadata.artist,
            song_title: Some(&metadata.title),
        })
        .await?
        .unwrap_or_else(||
     */

    let album_id = if let Some((album_uri, album_title)) = &release.album {
        Some(
            state
                .db
                .insert_album_if_not_exists(&DbAlbum {
                    album_id: uri_to_uuid(album_uri.as_str()),
                    uri: album_uri.to_string(),
                    title: album_title.clone(),
                    cover_art_id: None,
                })
                .await?,
        )
    } else {
        None
    };

    let artist_id = if let Some((artist_uri, artist_name)) = &release.artist {
        Some(
            state
                .db
                .insert_artist_if_not_exists(&DbArtist {
                    artist_id: uri_to_uuid(artist_uri.as_str()),
                    uri: artist_uri.to_string(),
                    name: artist_name.clone(),
                    cover_art_id: None,
                })
                .await?,
        )
    } else {
        None
    };

    let album_artist_id = if let Some((artist_uri, artist_name)) = &release.album_artist {
        Some(
            state
                .db
                .insert_artist_if_not_exists(&DbArtist {
                    artist_id: uri_to_uuid(artist_uri.as_str()),
                    uri: artist_uri.to_string(),
                    name: artist_name.clone(),
                    cover_art_id: None,
                })
                .await?,
        )
    } else {
        None
    };

    if let Some(album_id) = album_id {
        if let Some(actual_artist_id) = album_artist_id.or(artist_id) {
            state
                .db
                .upsert_album_artist(album_id, actual_artist_id)
                .await?;
        }
    }

    let (song_uri, song_title) = &release.song;
    let suffix = path.extension().and_then(|s| s.to_str());
    let content_type = match &suffix {
        Some("ogg") => Some("audio/ogg"),
        _ => None,
    };

    let song_id = Some(
        state
            .db
            .insert_song_if_not_exists(&DbSong {
                song_id: uri_to_uuid(song_uri.as_str()),
                uri: song_uri.to_string(),
                title: song_title.clone(),
                created: (state.now_provider)(),
                date: release.release_date,
                cover_art_id: None,
                artist_id,
                album_id,
                content_type: content_type.map(|s| s.to_owned()),
                suffix: suffix.map(|s| s.to_owned()),
                size: Some(file_size),
                track_number: metadata.track_number,
                disc_number: metadata.disc_number,
                duration: metadata.duration,
                bit_rate: metadata.bit_rate,
                genre: release.genre,
            })
            .await?,
    );

    state
        .db
        .insert_folder_child_if_not_exists(&DbFolderChild {
            folder_child_id: uri_to_uuid(folder_child_path.as_str()),
            folder_id,
            uri: folder_child_path.to_string(),
            path: folder_child_path,
            name: song_title.clone(),
            song_id,
        })
        .await?;

    Ok(())
}

pub async fn import_missing_album_cover_art(state: Arc<TaskState>) -> AppResult<()> {
    let results = sqlx::query(
        r#"
        SELECT albums.*, ar.name AS artist_name FROM albums
        LEFT JOIN album_artists aa on albums.album_id = aa.album_id
        LEFT JOIN artists ar on aa.artist_id = ar.artist_id
        WHERE albums.cover_art_id IS NULL
        "#,
    )
    .map(|row: SqliteRow| {
        let id: Uuid = row.get("album_id");
        let title: String = row.get("title");
        let name: String = row.get("artist_name");
        (id, name, title)
    })
    .fetch_all(state.db.conn().await?.deref_mut())
    .await?;

    let mut set = JoinSet::new();
    for (album_id, artist_name, album_title) in results {
        debug!(
            ?album_id,
            ?artist_name,
            ?album_title,
            "Trying to find album cover art"
        );
        let state = state.clone();
        set.spawn(async move {
            if let Some(url) = state
                .provider_list
                .find_cover_art(&FindCoverArtQuery {
                    album: Some(&album_title),
                    artist: Some(&artist_name),
                    song_title: None,
                })
                .await?
            {
                let cover_art_id = insert_cover_art(&state.db, &url).await?;

                // Update album and its songs
                let rows = sqlx::query("UPDATE albums SET cover_art_id = ? WHERE album_id = ?")
                    .bind(cover_art_id)
                    .bind(album_id)
                    .execute(state.db.conn().await?.deref_mut())
                    .await?;
                debug!("{:?} album rows affected", rows.rows_affected());
                let rows = sqlx::query(
                    "UPDATE songs SET cover_art_id = ? WHERE album_id = ? AND cover_art_id IS NULL",
                )
                .bind(cover_art_id)
                .bind(album_id)
                .execute(state.db.conn().await?.deref_mut())
                .await?;
                debug!("{:?} song rows affected", rows.rows_affected());
            }

            Ok(())
        });
    }

    await_join_set(set).await?;

    Ok(())
}

pub async fn import_missing_song_cover_art(state: Arc<TaskState>) -> AppResult<()> {
    let results = sqlx::query(
        r#"
        SELECT songs.song_id, songs.title, al.title AS album_title, ar.name AS artist_name FROM songs
        LEFT JOIN albums al on al.album_id = songs.album_id
        LEFT JOIN album_artists aa on al.album_id = aa.album_id
        LEFT JOIN artists ar on aa.artist_id = ar.artist_id
        WHERE songs.cover_art_id IS NULL
        "#,
    )
    .map(|row: SqliteRow| {
        let song_id: Uuid = row.get("song_id");
        let album_title: Option<String> = row.get("album_title");
        let title: String = row.get("title");
        let artist_name: Option<String> = row.get("artist_name");
        (song_id, album_title, title, artist_name)
    })
    .fetch_all(state.db.conn().await?.deref_mut())
    .await?;

    let mut set = JoinSet::new();
    for (song_id, album_title, song_title, artist_name) in results {
        debug!(
            ?song_id,
            ?song_title,
            ?album_title,
            ?artist_name,
            "Trying to find song cover art"
        );
        let state = state.clone();
        set.spawn(async move {
            if let Some(url) = state
                .provider_list
                .find_cover_art(&FindCoverArtQuery {
                    album: album_title.as_deref(),
                    artist: artist_name.as_deref(),
                    song_title: Some(song_title.as_str()),
                })
                .await?
            {
                let cover_art_id = insert_cover_art(&state.db, &url).await?;

                sqlx::query("UPDATE songs SET cover_art_id = ? WHERE song_id = ?")
                    .bind(cover_art_id)
                    .bind(song_id)
                    .execute(state.db.conn().await?.deref_mut())
                    .await?;
            }

            Ok(())
        });
    }

    await_join_set(set).await?;

    Ok(())
}

pub async fn import_missing_artist_cover_art(state: Arc<TaskState>) -> AppResult<()> {
    let results = sqlx::query(
        r#"
        SELECT al.title, artists.name AS artist_name, artists.artist_id FROM artists
        LEFT JOIN album_artists aa on artists.artist_id = aa.artist_id
        LEFT JOIN albums al on aa.album_id = al.album_id
        WHERE artists.cover_art_id IS NULL
        "#,
    )
    .map(|row: SqliteRow| {
        let id: Uuid = row.get("artist_id");
        let title: String = row.get("title");
        let name: String = row.get("artist_name");
        (id, name, title)
    })
    .fetch_all(state.db.conn().await?.deref_mut())
    .await?;

    let mut set = JoinSet::new();
    for (artist_id, artist_name, album_title) in results {
        debug!(
            ?artist_id,
            ?artist_name,
            ?album_title,
            "Trying to find artist cover art"
        );
        let state = state.clone();
        set.spawn(async move {
            if let Some(url) = state
                .provider_list
                .find_artist_photo(&FindCoverArtQuery {
                    album: Some(&album_title),
                    artist: Some(&artist_name),
                    song_title: None,
                })
                .await?
            {
                let cover_art_id = insert_cover_art(&state.db, &url).await?;
                // Update artist
                let rows = sqlx::query("UPDATE artists SET cover_art_id = ? WHERE artist_id = ?")
                    .bind(cover_art_id)
                    .bind(artist_id)
                    .execute(state.db.conn().await?.deref_mut())
                    .await?;
                debug!("{:?} artist rows affected", rows.rows_affected());
            }

            Ok(())
        });
    }

    await_join_set(set).await?;

    Ok(())
}

async fn insert_cover_art(db: &Db, url: &str) -> AppResult<Uuid> {
    let client = reqwest_client();

    // Find out the actual (potentially redirected) url first
    let head = client.head(url).send().await?;
    let url = head.url().to_string();

    // only cover the path in the UUID, since hostnames may differ sometimes due to CDNs etc
    let cover_art_id = uri_to_uuid(head.url().path());

    match db.find_cover_art(cover_art_id).await? {
        Some(id) => Ok(id),
        None => {
            let response = client.get(&url).send().await?;
            let data = response.bytes().await?.to_vec();
            Ok(db
                .insert_cover_art_if_not_exists(&DbCoverArt {
                    cover_art_id,
                    uri: url,
                    data,
                })
                .await?)
        }
    }
}

async fn await_join_set(mut set: JoinSet<AppResult<()>>) -> AppResult<()> {
    while let Some(result) = set.join_next().await {
        if let Err(e) = result? {
            error!(?e, "Error in background task");
        }
    }

    Ok(())
}
