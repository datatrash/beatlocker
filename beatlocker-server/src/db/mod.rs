mod db_pool;
mod model;

pub use model::*;
use std::fmt::{Debug, Formatter};
use std::ops::DerefMut;
use std::path::PathBuf;

use crate::AppResult;
use chrono::Duration;
use db_pool::DbPool;
use deadpool::managed::{Object, Pool};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow, SqliteSynchronous};
use sqlx::types::Uuid;
use sqlx::Row;
use std::str::FromStr;
use tracing::debug;

pub struct Db {
    pool: Pool<DbPool>,
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
    pub fn new(options: &DatabaseOptions) -> AppResult<Self> {
        let connect_options = if options.in_memory {
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
                .synchronous(SqliteSynchronous::Normal)
                .journal_mode(SqliteJournalMode::Wal)
                .busy_timeout(std::time::Duration::from_secs(30))
        };
        let mgr = DbPool::new(connect_options);
        let pool: Pool<DbPool> = Pool::builder(mgr).build()?;

        Ok(Db { pool })
    }

    pub async fn migrate(&self) -> AppResult<()> {
        sqlx::migrate!("./migrations")
            .run(self.pool.get().await?.deref_mut())
            .await?;
        Ok(())
    }

    pub async fn conn(&self) -> AppResult<Object<DbPool>> {
        Ok(self.pool.get().await?)
    }

    pub async fn update_last_updated(&self, folder_child_id: Uuid) -> AppResult<()> {
        let timestamp = chrono::offset::Utc::now();
        sqlx::query("UPDATE folder_children SET last_updated = ? WHERE folder_child_id = ?")
            .bind(timestamp)
            .bind(folder_child_id)
            .execute(self.conn().await?.deref_mut())
            .await?;

        Ok(())
    }

    pub async fn insert_album_if_not_exists(&self, album: &DbAlbum) -> AppResult<Uuid> {
        debug!(?album, "Inserting album");

        let id = sqlx::query(
            r#"
        INSERT INTO albums (album_id, title, cover_art_id)
        VALUES (?, ?, ?)
        ON CONFLICT (album_id) DO UPDATE set album_id = album_id
        RETURNING album_id
        "#,
        )
        .bind(album.album_id)
        .bind(&album.title)
        .bind(album.cover_art_id)
        .map(|row| row.get("album_id"))
        .fetch_one(self.conn().await?.deref_mut())
        .await?;

        Ok(id)
    }

    pub async fn insert_artist_if_not_exists(&self, artist: &DbArtist) -> AppResult<Uuid> {
        debug!(?artist, "Inserting artist");

        let id = sqlx::query(
            r#"
        INSERT INTO artists (artist_id, name, cover_art_id)
        VALUES (?, ?, ?)
        ON CONFLICT (artist_id) DO UPDATE set artist_id = artist_id
        RETURNING artist_id
        "#,
        )
        .bind(artist.artist_id)
        .bind(&artist.name)
        .bind(artist.cover_art_id)
        .map(|row| row.get("artist_id"))
        .fetch_one(self.conn().await?.deref_mut())
        .await?;

        Ok(id)
    }

    pub async fn upsert_album_artist(&self, album_id: Uuid, artist_id: Uuid) -> AppResult<()> {
        sqlx::query(
            r#"
        INSERT OR IGNORE INTO album_artists (album_id, artist_id)
        VALUES (?, ?)
        "#,
        )
        .bind(album_id)
        .bind(artist_id)
        .execute(self.conn().await?.deref_mut())
        .await?;

        Ok(())
    }

    pub async fn find_song_by_id(&self, id: Uuid) -> AppResult<Option<DbSong>> {
        let result = sqlx::query("SELECT * FROM songs WHERE song_id = ?")
            .bind(id)
            .map(|row: SqliteRow| {
                let duration: Option<u32> = row.get("duration");
                DbSong {
                    song_id: row.get("song_id"),
                    title: row.get("title"),
                    created: row.get("created"),
                    date: row.get("date"),
                    cover_art_id: row.get("cover_art_id"),
                    artist_id: row.get("artist_id"),
                    album_id: row.get("album_id"),
                    content_type: row.get("content_type"),
                    suffix: row.get("suffix"),
                    size: row.get("size"),
                    track_number: row.get("track_number"),
                    disc_number: row.get("disc_number"),
                    duration: duration.map(|secs| Duration::seconds(secs as i64)),
                    bit_rate: row.get("bit_rate"),
                    genre: row.get("genre"),
                }
            })
            .fetch_optional(self.conn().await?.deref_mut())
            .await?;

        Ok(result)
    }

    pub async fn find_artist_by_id(&self, id: Uuid) -> AppResult<Option<DbArtist>> {
        let result = sqlx::query("SELECT * FROM artists WHERE artist_id = ?")
            .bind(id)
            .map(|row: SqliteRow| DbArtist {
                artist_id: row.get("artist_id"),
                name: row.get("name"),
                cover_art_id: row.get("cover_art_id"),
            })
            .fetch_optional(self.conn().await?.deref_mut())
            .await?;

        Ok(result)
    }

    pub async fn find_album_by_id(&self, id: Uuid) -> AppResult<Option<DbAlbum>> {
        let result = sqlx::query("SELECT * FROM albums WHERE album_id = ?")
            .bind(id)
            .map(|row: SqliteRow| DbAlbum {
                album_id: row.get("album_id"),
                title: row.get("title"),
                cover_art_id: row.get("cover_art_id"),
            })
            .fetch_optional(self.conn().await?.deref_mut())
            .await?;

        Ok(result)
    }

    pub async fn find_folder_child_by_path(&self, path: &str) -> AppResult<Option<Uuid>> {
        let result = sqlx::query("SELECT folder_child_id FROM folder_children WHERE path = ?")
            .bind(path)
            .map(|row: SqliteRow| row.get("folder_child_id"))
            .fetch_optional(self.conn().await?.deref_mut())
            .await?;

        Ok(result)
    }

    pub async fn find_cover_art(&self, cover_art_id: Uuid) -> AppResult<Option<Uuid>> {
        let result = sqlx::query("SELECT cover_art_id FROM cover_art WHERE cover_art_id = ?")
            .bind(cover_art_id)
            .map(|row: SqliteRow| row.get("cover_art_id"))
            .fetch_optional(self.conn().await?.deref_mut())
            .await?;

        Ok(result)
    }

    pub async fn insert_folder_if_not_exists(&self, folder: &DbFolder) -> AppResult<Uuid> {
        debug!(?folder, "Trying to insert folder");

        let id = sqlx::query(
            r#"
        INSERT INTO folders (folder_id, parent_id, name, cover_art_id, created)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT (folder_id) DO UPDATE set folder_id = folder_id
        RETURNING folder_id
        "#,
        )
        .bind(folder.folder_id)
        .bind(folder.parent_id)
        .bind(&folder.name)
        .bind(folder.cover_art_id)
        .bind(folder.created)
        .map(|row| row.get("folder_id"))
        .fetch_one(self.conn().await?.deref_mut())
        .await?;

        Ok(id)
    }

    pub async fn insert_folder_child_if_not_exists(
        &self,
        child: &DbFolderChild,
    ) -> AppResult<Uuid> {
        debug!(?child, "Trying to insert folder child");

        let id = sqlx::query(
            r#"
        INSERT INTO folder_children (folder_child_id, folder_id, path, name, song_id)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT (folder_child_id) DO UPDATE set folder_child_id = folder_child_id
        RETURNING folder_child_id
        "#,
        )
        .bind(child.folder_child_id)
        .bind(child.folder_id)
        .bind(&child.path)
        .bind(&child.name)
        .bind(child.song_id)
        .map(|row| row.get("folder_child_id"))
        .fetch_one(self.conn().await?.deref_mut())
        .await?;

        Ok(id)
    }

    pub async fn insert_song_if_not_exists(&self, song: &DbSong) -> AppResult<Uuid> {
        debug!(?song, "Trying to insert song");

        let id = sqlx::query(
            r#"
        INSERT INTO songs (song_id, title, created, date, cover_art_id, artist_id, album_id, content_type, suffix, size, track_number, disc_number, duration, bit_rate)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (song_id) DO UPDATE set song_id = song_id
        RETURNING song_id
        "#,
        )
            .bind(song.song_id)
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
        .fetch_one(self.conn().await?.deref_mut())
        .await?;

        Ok(id)
    }

    pub async fn insert_cover_art_if_not_exists(&self, cover_art: &DbCoverArt) -> AppResult<Uuid> {
        let cover_art_id = cover_art.cover_art_id.to_string();
        debug!(cover_art_id, "Inserting cover art");

        let id = sqlx::query(
            r#"
        INSERT INTO cover_art (cover_art_id, data)
        VALUES (?, ?)
        ON CONFLICT (cover_art_id) DO UPDATE set cover_art_id = cover_art_id
        RETURNING cover_art_id
        "#,
        )
        .bind(cover_art.cover_art_id)
        .bind(&cover_art.data)
        .map(|row| row.get("cover_art_id"))
        .fetch_one(self.conn().await?.deref_mut())
        .await?;

        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn can_migrate() -> AppResult<()> {
        let db = Db::new(&DatabaseOptions {
            path: None,
            in_memory: true,
        })?;
        db.migrate().await?;
        Ok(())
    }
}
