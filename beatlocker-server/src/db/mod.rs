mod model;

pub use model::*;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;

use crate::AppResult;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteRow};
use sqlx::types::Uuid;
use sqlx::{Pool, Row, Sqlite};
use std::str::FromStr;
use tracing::debug;

pub struct Db {
    pool: Pool<Sqlite>,
}

impl Debug for Db {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("[Db]")
    }
}

#[derive(Clone, Debug)]
pub struct DatabaseOptions {
    pub path: Option<PathBuf>,
    pub in_memory: bool,
}

impl Db {
    pub async fn new(options: &DatabaseOptions) -> AppResult<Self> {
        let pool_options = if options.in_memory {
            SqliteConnectOptions::from_str("sqlite::memory:")?
        } else {
            SqliteConnectOptions::new()
                .filename(
                    options
                        .path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join("sqlite.db"),
                )
                .create_if_missing(true)
        };
        let pool = Pool::<Sqlite>::connect_with(pool_options).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Db { pool })
    }

    pub async fn conn(&self) -> AppResult<PoolConnection<Sqlite>> {
        Ok(self.pool.acquire().await?)
    }

    pub async fn insert_album_if_not_exists(&self, album: &DbAlbum) -> AppResult<Uuid> {
        debug!(?album, "Inserting album");
        let mut conn = self.pool.acquire().await?;

        let id = sqlx::query(
            r#"
        INSERT INTO albums (album_id, uri, title, cover_art_id)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (uri) DO UPDATE set uri = uri
        RETURNING album_id
        "#,
        )
        .bind(album.album_id)
        .bind(&album.uri)
        .bind(&album.title)
        .bind(album.cover_art_id)
        .map(|row| row.get("album_id"))
        .fetch_one(&mut conn)
        .await?;

        Ok(id)
    }

    pub async fn insert_artist_if_not_exists(&self, artist: &DbArtist) -> AppResult<Uuid> {
        debug!(?artist, "Inserting artist");
        let mut conn = self.pool.acquire().await?;

        let id = sqlx::query(
            r#"
        INSERT INTO artists (artist_id, uri, name, cover_art_id)
        VALUES (?, ?, ?, ?)
        ON CONFLICT (uri) DO UPDATE set uri = uri
        RETURNING artist_id
        "#,
        )
        .bind(artist.artist_id)
        .bind(&artist.uri)
        .bind(&artist.name)
        .bind(artist.cover_art_id)
        .map(|row| row.get("artist_id"))
        .fetch_one(&mut conn)
        .await?;

        Ok(id)
    }

    pub async fn upsert_album_artist(&self, album_id: Uuid, artist_id: Uuid) -> AppResult<()> {
        let mut conn = self.pool.acquire().await?;

        sqlx::query(
            r#"
        INSERT OR IGNORE INTO album_artists (album_id, artist_id)
        VALUES (?, ?)
        "#,
        )
        .bind(album_id)
        .bind(artist_id)
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn find_folder_by_uri(&self, uri: &str) -> AppResult<Option<Uuid>> {
        let mut conn = self.pool.acquire().await?;

        let result = sqlx::query("SELECT folder_id FROM folders WHERE uri = ?")
            .bind(uri)
            .map(|row: SqliteRow| row.get("folder_id"))
            .fetch_optional(&mut conn)
            .await?;

        Ok(result)
    }

    pub async fn find_folder_child_by_path(&self, path: &str) -> AppResult<Option<Uuid>> {
        let mut conn = self.pool.acquire().await?;

        let result = sqlx::query("SELECT folder_child_id FROM folder_children WHERE path = ?")
            .bind(path)
            .map(|row: SqliteRow| row.get("folder_child_id"))
            .fetch_optional(&mut conn)
            .await?;

        Ok(result)
    }

    pub async fn find_cover_art(&self, cover_art_id: Uuid) -> AppResult<Option<Uuid>> {
        let mut conn = self.pool.acquire().await?;

        let result = sqlx::query("SELECT cover_art_id FROM cover_art WHERE cover_art_id = ?")
            .bind(cover_art_id)
            .map(|row: SqliteRow| row.get("cover_art_id"))
            .fetch_optional(&mut conn)
            .await?;

        Ok(result)
    }

    pub async fn insert_folder_if_not_exists(&self, folder: &DbFolder) -> AppResult<Uuid> {
        debug!(?folder, "Trying to insert folder");

        let mut conn = self.pool.acquire().await?;

        let id = sqlx::query(
            r#"
        INSERT INTO folders (folder_id, parent_id, uri, name, cover_art_id, created)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT (uri) DO UPDATE set uri = uri
        RETURNING folder_id
        "#,
        )
        .bind(folder.folder_id)
        .bind(folder.parent_id)
        .bind(&folder.uri)
        .bind(&folder.name)
        .bind(folder.cover_art_id)
        .bind(folder.created)
        .map(|row| row.get("folder_id"))
        .fetch_one(&mut conn)
        .await?;

        Ok(id)
    }

    pub async fn insert_folder_child_if_not_exists(
        &self,
        child: &DbFolderChild,
    ) -> AppResult<Uuid> {
        debug!(?child, "Trying to insert folder child");

        let mut conn = self.pool.acquire().await?;

        let id = sqlx::query(
            r#"
        INSERT INTO folder_children (folder_child_id, folder_id, uri, path, name, song_id)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT (uri) DO UPDATE set uri = uri
        RETURNING folder_child_id
        "#,
        )
        .bind(child.folder_child_id)
        .bind(child.folder_id)
        .bind(&child.uri)
        .bind(&child.path)
        .bind(&child.name)
        .bind(child.song_id)
        .map(|row| row.get("folder_child_id"))
        .fetch_one(&mut conn)
        .await?;

        Ok(id)
    }

    pub async fn insert_song_if_not_exists(&self, song: &DbSong) -> AppResult<Uuid> {
        debug!(?song, "Trying to insert song");

        let mut conn = self.pool.acquire().await?;

        let id = sqlx::query(
            r#"
        INSERT INTO songs (song_id, uri, title, created, date, cover_art_id, artist_id, album_id, content_type, suffix, size, track_number, disc_number, duration, bit_rate)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (uri) DO UPDATE set uri = uri
        RETURNING song_id
        "#,
        )
            .bind(song.song_id)
            .bind(&song.uri)
            .bind(&song.title)
            .bind(song.created)
            .bind(song.date)
            .bind(song.cover_art_id)
            .bind(song.artist_id)
            .bind(song.album_id)
            .bind(&song.content_type)
            .bind(&song.suffix)
            .bind(song.size)
            .bind(song.track_number)
            .bind(song.disc_number)
            .bind(song.duration.map(|d| d.num_seconds()))
            .bind(song.bit_rate)
            .map(|row| row.get("song_id"))
        .fetch_one(&mut conn)
        .await?;

        Ok(id)
    }

    pub async fn insert_cover_art_if_not_exists(&self, cover_art: &DbCoverArt) -> AppResult<Uuid> {
        let uri = &cover_art.uri;
        debug!(?uri, "Inserting cover art");
        let mut conn = self.pool.acquire().await?;

        let id = sqlx::query(
            r#"
        INSERT INTO cover_art (cover_art_id, uri, data)
        VALUES (?, ?, ?)
        ON CONFLICT (cover_art_id) DO UPDATE set cover_art_id = cover_art_id
        ON CONFLICT (uri) DO UPDATE set uri = uri
        RETURNING cover_art_id
        "#,
        )
        .bind(cover_art.cover_art_id)
        .bind(&cover_art.uri)
        .bind(&cover_art.data)
        .map(|row| row.get("cover_art_id"))
        .fetch_one(&mut conn)
        .await?;

        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn can_migrate() -> AppResult<()> {
        let _db = Db::new(&DatabaseOptions {
            path: None,
            in_memory: true,
        })
        .await?;
        Ok(())
    }
}
