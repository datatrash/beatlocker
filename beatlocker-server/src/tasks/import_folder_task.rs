use super::*;
use crate::db::{DbAlbum, DbArtist, DbFailedFolderChild, DbFolder, DbFolderChild, DbSong};
use crate::str_to_uuid;
use crate::tasks::extract_metadata::extract_metadata;
use async_recursion::async_recursion;
use std::path::Path;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_stream::wrappers::ReadDirStream;
use tokio_stream::StreamExt;
use tracing::{debug, warn};

#[async_recursion]
pub async fn import_folder(
    state: Arc<TaskState>,
    folder: &Path,
    parent_folder_id: Option<Uuid>,
) -> AppResult<()> {
    debug!(?folder, "Processing folder");

    let parent_folder_id = match parent_folder_id {
        Some(p) => p,
        None => {
            state
                .db
                .insert_folder_if_not_exists(&DbFolder {
                    folder_id: Uuid::nil(),
                    parent_id: None,
                    name: "root".to_owned(),
                    cover_art_id: None,
                    created: (state.options.now_provider)(),
                })
                .await?
        }
    };

    // Insert folder in DB
    let folder_id = if folder == state.options.path {
        Uuid::nil()
    } else {
        let folder_name = folder.file_name().unwrap();

        state
            .db
            .insert_folder_if_not_exists(&DbFolder {
                folder_id: str_to_uuid(folder.to_str().unwrap()),
                parent_id: Some(parent_folder_id),
                name: folder_name.to_string_lossy().to_string(),
                cover_art_id: None,
                created: (state.options.now_provider)(),
            })
            .await?
    };

    let read_dir_chunks = ReadDirStream::new(tokio::fs::read_dir(folder).await?)
        .chunks_timeout(64, Duration::from_secs(10));
    tokio::pin!(read_dir_chunks);

    while let Some(chunk) = read_dir_chunks.next().await {
        let mut set = JoinSet::new();
        for entry in chunk.into_iter().flatten() {
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                let state = state.clone();
                let entry = entry.path().clone();
                let folder_id = folder_id;
                set.spawn(async move {
                    let _ = import_folder(state, entry.as_path(), Some(folder_id)).await;
                    Ok(())
                });
            }
            if file_type.is_file() {
                let folder_id = folder_id;
                let state = state.clone();
                let entry = entry.path().clone();
                set.spawn(async move { import_file(state, entry.as_path(), folder_id).await });
            }
        }

        await_join_set(set).await?;
    }

    debug!(?folder, "Processing folder done");
    Ok(())
}

async fn import_file(state: Arc<TaskState>, path: &Path, folder_id: Uuid) -> AppResult<()> {
    let folder_child_path = path.to_str().unwrap().to_string();

    if state
        .db
        .find_failed_folder_child_by_path(&folder_child_path)
        .await?
        .is_some()
    {
        debug!(?path, "Previously failed");
        return Ok(());
    }

    if state
        .db
        .find_folder_child_by_path(&folder_child_path)
        .await?
        .is_some()
    {
        debug!(?path, "Already imported");
        return Ok(());
    }

    let filename = match path.file_name() {
        Some(f) => f,
        None => {
            debug!(?path, "Could not determine filename");
            return Ok(());
        }
    };

    info!(?path, "Importing file");
    let (metadata, file_size) = {
        let file = std::fs::File::open(path)?;
        let len = file.metadata()?.len() as u32;
        (
            match extract_metadata(filename, || Box::new(file.try_clone().unwrap())) {
                Ok(m) => m,
                Err(e) => {
                    warn!(?path, ?e, "Could not extract metadata");
                    None
                }
            },
            len,
        )
    };

    let failed = match &metadata {
        Some(m) => !m.is_valid(),
        None => true,
    };
    if failed {
        warn!(?path, "File or extracted metadata is not valid");

        state
            .db
            .insert_failed_folder_child_if_not_exists(&DbFailedFolderChild {
                folder_child_id: str_to_uuid(folder_child_path.as_str()),
                folder_id,
                path: folder_child_path,
            })
            .await?;

        return Ok(());
    }
    let metadata = metadata.unwrap();

    let album_id = if let Some(album_title) = &metadata.album {
        let artist = metadata
            .album_artist
            .clone()
            .unwrap_or_else(|| metadata.artist().to_string());

        let album_id = str_to_uuid(&format!("{}{}", album_title, artist));
        Some(
            state
                .db
                .insert_album_if_not_exists(&DbAlbum {
                    album_id,
                    title: album_title.clone(),
                    cover_art_id: None,
                })
                .await?,
        )
    } else {
        None
    };

    let artist_id = Some(
        state
            .db
            .insert_artist_if_not_exists(&DbArtist {
                artist_id: str_to_uuid(metadata.artist()),
                name: metadata.artist().to_string(),
                cover_art_id: None,
                musicbrainz_id: None,
            })
            .await?,
    );

    let album_artist_id = if let Some(artist_name) = &metadata.album_artist {
        Some(
            state
                .db
                .insert_artist_if_not_exists(&DbArtist {
                    artist_id: str_to_uuid(artist_name.as_str()),
                    name: artist_name.clone(),
                    cover_art_id: None,
                    musicbrainz_id: None,
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

    let song_title = &metadata.title.unwrap();

    let song_id = str_to_uuid(&format!(
        "{}{}{}",
        song_title,
        artist_id.unwrap_or_default(),
        album_id.unwrap_or_default()
    ));
    let song_id = Some(
        state
            .db
            .insert_song_if_not_exists(&DbSong {
                song_id,
                title: song_title.clone(),
                created: (state.options.now_provider)(),
                date: metadata.date,
                cover_art_id: None,
                artist_id,
                album_id,
                content_type: metadata.content_type,
                suffix: metadata.suffix,
                size: Some(file_size),
                track_number: metadata.track_number,
                disc_number: metadata.disc_number,
                duration: metadata.duration,
                bit_rate: metadata.bit_rate,
                genre: metadata.genre,
            })
            .await?,
    );

    state
        .db
        .insert_folder_child_if_not_exists(&DbFolderChild {
            folder_child_id: str_to_uuid(folder_child_path.as_str()),
            folder_id,
            path: folder_child_path,
            name: song_title.clone(),
            song_id,
            last_updated: None,
        })
        .await?;

    Ok(())
}
