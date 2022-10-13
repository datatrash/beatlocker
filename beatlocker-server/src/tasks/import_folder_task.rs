use super::*;
use crate::db::{DbAlbum, DbArtist, DbFolder, DbFolderChild, DbSong};
use crate::tasks::extract_metadata::extract_metadata;
use crate::uri_to_uuid;
use async_recursion::async_recursion;
use std::path::Path;
use tokio::task::JoinSet;
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
                    uri: "root".to_owned(),
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
        let folder_uri = format!("path:{}", folder.to_str().unwrap());

        state
            .db
            .insert_folder_if_not_exists(&DbFolder {
                folder_id: uri_to_uuid(&folder_uri),
                parent_id: Some(parent_folder_id),
                uri: folder_uri.clone(),
                name: folder_name.to_string_lossy().to_string(),
                cover_art_id: None,
                created: (state.options.now_provider)(),
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
                let folder_id = folder_id;
                set.spawn(async move {
                    let _ = import_folder(state, entry.as_path(), Some(folder_id)).await;
                    Ok(())
                });
            }
            if file_type.is_file()
                && entry
                    .path()
                    .extension()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    == Some("ogg".to_string())
            {
                let folder_id = folder_id;
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

    info!(?path, "Importing file");
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

    let album_id = if let Some(album_title) = &metadata.album {
        Some(
            state
                .db
                .insert_album_if_not_exists(&DbAlbum {
                    album_id: uri_to_uuid(album_title.as_str()),
                    uri: album_title.clone(),
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
                artist_id: uri_to_uuid(&metadata.artist),
                uri: metadata.artist.clone(),
                name: metadata.artist.clone(),
                cover_art_id: None,
            })
            .await?,
    );

    let album_artist_id = if let Some(artist_name) = &metadata.album_artist {
        Some(
            state
                .db
                .insert_artist_if_not_exists(&DbArtist {
                    artist_id: uri_to_uuid(artist_name.as_str()),
                    uri: artist_name.to_string(),
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

    let song_title = &metadata.title;
    let suffix = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());
    let content_type = match &suffix {
        Some(ct) => match ct.as_str() {
            "ogg" => Some("audio/ogg"),
            _ => None,
        },
        _ => None,
    };

    let song_id = Some(
        state
            .db
            .insert_song_if_not_exists(&DbSong {
                song_id: uri_to_uuid(song_title.as_str()),
                uri: song_title.to_string(),
                title: song_title.clone(),
                created: (state.options.now_provider)(),
                date: metadata.date,
                cover_art_id: None,
                artist_id,
                album_id,
                content_type: content_type.map(|s| s.to_owned()),
                suffix,
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
