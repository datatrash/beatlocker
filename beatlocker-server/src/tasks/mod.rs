use crate::db::{DbAlbum, DbArtist, DbCoverArt, DbFolder, DbFolderChild, DbSong};
use crate::tasks::extract_metadata::extract_metadata;
use crate::{uri_to_uuid, AppResult, Db};

use crate::tasks::providers::{
    FindCoverArtQuery, FindReleaseQuery, InfoProvider, InfoProviderList, InfoProviderOptions,
};
use async_recursion::async_recursion;
use chrono::{DateTime, Utc};
use itertools::Itertools;
use reqwest::Client;
use sqlx::sqlite::SqliteRow;
use sqlx::types::Uuid;
use sqlx::Row;
use std::ffi::OsStr;
use std::fmt::{Debug, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::Receiver;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

mod extract_metadata;
mod providers;

#[derive(Debug)]
pub enum TaskMessage {
    ImportFolder(Arc<Db>, ImportFolderOptions),
    ImportMissingAlbumCoverArt(Arc<Db>, ImportMissingCoverArtOptions),
    ImportMissingArtistCoverArt(Arc<Db>, ImportMissingCoverArtOptions),
    ImportMissingSongCoverArt(Arc<Db>, ImportMissingCoverArtOptions),
    Shutdown,
}

pub async fn start_task_runner() -> AppResult<(Sender<TaskMessage>, Receiver<()>)> {
    let (tx, mut rx) = mpsc::channel(32);
    let (done_tx, done_rx) = oneshot::channel();

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let result = match message {
                TaskMessage::ImportFolder(db, options) => import_folder(&db, options).await,
                TaskMessage::ImportMissingAlbumCoverArt(db, options) => {
                    let provider_list = Arc::new(InfoProviderList::new(&InfoProviderOptions {
                        discogs_token: options.discogs_token.clone(),
                    }));

                    import_missing_album_cover_art(&db, provider_list).await
                }
                TaskMessage::ImportMissingArtistCoverArt(db, options) => {
                    let provider_list = Arc::new(InfoProviderList::new(&InfoProviderOptions {
                        discogs_token: options.discogs_token.clone(),
                    }));

                    import_missing_artist_cover_art(&db, provider_list).await
                }
                TaskMessage::ImportMissingSongCoverArt(db, options) => {
                    let provider_list = Arc::new(InfoProviderList::new(&InfoProviderOptions {
                        discogs_token: options.discogs_token.clone(),
                    }));

                    import_missing_song_cover_art(&db, provider_list).await
                }
                TaskMessage::Shutdown => {
                    info!("Shutting down background tasks...");
                    let _ = done_tx.send(());
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

    Ok((tx, done_rx))
}

pub struct ImportFolderOptions {
    pub root_path: PathBuf,
    pub discogs_token: Option<String>,
    pub now_provider: Arc<Box<dyn Fn() -> DateTime<Utc> + Send + Sync>>,
}

impl Debug for ImportFolderOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("[ImportFolderOptions")
    }
}

pub async fn import_folder(db: &Db, options: ImportFolderOptions) -> AppResult<()> {
    let provider_list = Arc::new(InfoProviderList::new(&InfoProviderOptions {
        discogs_token: options.discogs_token.clone(),
    }));

    db.insert_folder_if_not_exists(&DbFolder {
        folder_id: Uuid::nil(),
        parent_id: None,
        uri: "root".to_owned(),
        name: "root".to_owned(),
        cover_art_id: None,
        created: (options.now_provider)(),
    })
    .await?;
    import_folder_impl(db, provider_list, &options, &options.root_path, Uuid::nil()).await?;
    Ok(())
}

#[async_recursion]
async fn import_folder_impl(
    db: &Db,
    provider_list: Arc<InfoProviderList>,
    options: &ImportFolderOptions,
    folder: &Path,
    parent_folder_id: Uuid,
) -> AppResult<()> {
    debug!(?folder, "Processing folder");
    let extension = OsStr::new("ogg");

    let mut dirs = vec![];
    let mut files = vec![];
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            dirs.push(entry.path());
        }
        if file_type.is_file() && entry.path().extension() == Some(extension) {
            files.push(entry.path());
        }
    }

    // Insert folder in DB
    let folder_id = if folder == options.root_path {
        Uuid::nil()
    } else {
        let folder_name = folder.file_name().unwrap();
        let folder_uri = format!("path:{}", folder.to_str().unwrap());

        db.insert_folder_if_not_exists(&DbFolder {
            folder_id: uri_to_uuid(&folder_uri),
            parent_id: Some(parent_folder_id),
            uri: folder_uri.clone(),
            name: folder_name.to_string_lossy().to_string(),
            cover_art_id: None,
            created: (options.now_provider)(),
        })
        .await?
    };

    for file in files.into_iter().sorted() {
        import_file(
            db,
            provider_list.clone(),
            options,
            file.as_path(),
            folder_id,
        )
        .await?;
    }
    for dir in dirs.into_iter().sorted() {
        import_folder_impl(db, provider_list.clone(), options, dir.as_path(), folder_id).await?;
    }

    debug!(?folder, "Processing folder done");
    Ok(())
}

async fn import_file<'a>(
    db: &'a Db,
    provider_list: Arc<InfoProviderList>,
    options: &'a ImportFolderOptions,
    path: &'a Path,
    folder_id: Uuid,
) -> AppResult<()> {
    let folder_child_path = path.to_str().unwrap().to_string();
    if db
        .find_folder_child_by_path(&folder_child_path)
        .await?
        .is_some()
    {
        return Ok(());
    }

    debug!(?path, "Importing file");
    let file = std::fs::File::open(path)?;
    let metadata = extract_metadata(&file)?;
    if metadata.is_none() {
        warn!(?path, "Could not extract metadata");
        return Err(anyhow::format_err!("Could not extract metadata").into());
    }
    let metadata = metadata.unwrap();

    if let Some(release) = provider_list
        .find_release(&FindReleaseQuery {
            album: metadata.album.as_deref(),
            artist: &metadata.artist,
            song_title: Some(&metadata.title),
        })
        .await?
    {
        let album_id = if let Some((album_uri, album_title)) = &release.album {
            Some(
                db.insert_album_if_not_exists(&DbAlbum {
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
                db.insert_artist_if_not_exists(&DbArtist {
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
                db.insert_artist_if_not_exists(&DbArtist {
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
                db.upsert_album_artist(album_id, actual_artist_id).await?;
            }
        }

        let (song_uri, song_title) = &release.song;
        let suffix = path.extension().and_then(|s| s.to_str());
        let content_type = match &suffix {
            Some("ogg") => Some("audio/ogg"),
            _ => None,
        };

        let song_id = Some(
            db.insert_song_if_not_exists(&DbSong {
                song_id: uri_to_uuid(song_uri.as_str()),
                uri: song_uri.to_string(),
                title: song_title.clone(),
                created: (options.now_provider)(),
                date: release.release_date,
                cover_art_id: None,
                artist_id,
                album_id,
                content_type: content_type.map(|s| s.to_owned()),
                suffix: suffix.map(|s| s.to_owned()),
                size: Some(file.metadata()?.len() as u32),
                track_number: metadata.track_number,
                disc_number: metadata.disc_number,
                duration: metadata.duration,
                bit_rate: metadata.bit_rate,
                genre: release.genre,
            })
            .await?,
        );

        db.insert_folder_child_if_not_exists(&DbFolderChild {
            folder_child_id: uri_to_uuid(song_uri.as_str()),
            folder_id,
            uri: song_uri.to_string(),
            path: folder_child_path,
            name: song_title.clone(),
            song_id,
        })
        .await?;
    }

    Ok(())
}

#[derive(Clone)]
pub struct ImportMissingCoverArtOptions {
    pub discogs_token: Option<String>,
}

impl Debug for ImportMissingCoverArtOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("[ImportMissingCoverArtOptions]")
    }
}

pub async fn import_missing_album_cover_art(
    db: &Db,
    provider_list: Arc<InfoProviderList>,
) -> AppResult<()> {
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
    .fetch_all(&mut db.conn().await?)
    .await?;

    for (album_id, artist_name, album_title) in results {
        debug!(
            ?album_id,
            ?artist_name,
            ?album_title,
            "Trying to find album cover art"
        );
        if let Some(url) = provider_list
            .find_cover_art(&FindCoverArtQuery {
                album: Some(&album_title),
                artist: Some(&artist_name),
                song_title: None,
            })
            .await?
        {
            let cover_art_id = insert_cover_art(db, &url).await?;

            // Update album and its songs
            let rows = sqlx::query("UPDATE albums SET cover_art_id = ? WHERE album_id = ?")
                .bind(cover_art_id)
                .bind(album_id)
                .execute(&mut db.conn().await?)
                .await?;
            debug!("{:?} album rows affected", rows.rows_affected());
            let rows = sqlx::query(
                "UPDATE songs SET cover_art_id = ? WHERE album_id = ? AND cover_art_id IS NULL",
            )
            .bind(cover_art_id)
            .bind(album_id)
            .execute(&mut db.conn().await?)
            .await?;
            debug!("{:?} song rows affected", rows.rows_affected());
        }
    }

    Ok(())
}

pub async fn import_missing_song_cover_art(
    db: &Db,
    provider_list: Arc<InfoProviderList>,
) -> AppResult<()> {
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
    .fetch_all(&mut db.conn().await?)
    .await?;

    for (song_id, album_title, song_title, artist_name) in results {
        debug!(
            ?song_id,
            ?song_title,
            ?album_title,
            ?artist_name,
            "Trying to find song cover art"
        );
        if let Some(url) = provider_list
            .find_cover_art(&FindCoverArtQuery {
                album: album_title.as_deref(),
                artist: artist_name.as_deref(),
                song_title: Some(song_title.as_str()),
            })
            .await?
        {
            let cover_art_id = insert_cover_art(db, &url).await?;

            sqlx::query("UPDATE songs SET cover_art_id = ? WHERE song_id = ?")
                .bind(cover_art_id)
                .bind(song_id)
                .execute(&mut db.conn().await?)
                .await?;
        }
    }

    Ok(())
}

pub async fn import_missing_artist_cover_art(
    db: &Db,
    provider_list: Arc<InfoProviderList>,
) -> AppResult<()> {
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
    .fetch_all(&mut db.conn().await?)
    .await?;

    for (artist_id, artist_name, album_title) in results {
        debug!(
            ?artist_id,
            ?artist_name,
            ?album_title,
            "Trying to find artist cover art"
        );
        if let Some(url) = provider_list
            .find_artist_photo(&FindCoverArtQuery {
                album: Some(&album_title),
                artist: Some(&artist_name),
                song_title: None,
            })
            .await?
        {
            let cover_art_id = insert_cover_art(db, &url).await?;
            // Update artist
            let rows = sqlx::query("UPDATE artists SET cover_art_id = ? WHERE artist_id = ?")
                .bind(cover_art_id)
                .bind(artist_id)
                .execute(&mut db.conn().await?)
                .await?;
            debug!("{:?} artist rows affected", rows.rows_affected());
        }
    }

    Ok(())
}

async fn insert_cover_art(db: &Db, url: &str) -> AppResult<Uuid> {
    let client = Client::builder().build()?;

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
