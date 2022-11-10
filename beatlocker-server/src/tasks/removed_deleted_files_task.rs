use crate::{AppResult, TaskState};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

pub async fn remove_deleted_files(state: Arc<TaskState>) -> AppResult<()> {
    let mut conn = state.db.conn().await?;

    let folders = sqlx::query(
        r#"
        SELECT f.folder_id, f.path
        FROM folders f
        WHERE parent_id IS NOT NULL
        ORDER BY f.path
    "#,
    )
    .map(|row: SqliteRow| {
        let id: Uuid = row.get("folder_id");
        let path: String = row.get("path");
        (id, PathBuf::from_str(&path).unwrap())
    })
    .fetch_all(conn.deref_mut())
    .await?;

    for (folder_id, folder_path) in folders {
        let folder_deleted = tokio::fs::metadata(&folder_path).await.ok().is_none();

        if folder_deleted {
            info!("Folder was removed: {:?}", folder_path.as_os_str());
        }

        let children = sqlx::query(
            r#"
                        SELECT fc.folder_child_id, fc.path, fc.song_id
                        FROM folder_children fc
                        WHERE fc.folder_id = ?
                        ORDER BY fc.path
                    "#,
        )
        .bind(folder_id)
        .map(|row: SqliteRow| {
            let id: Uuid = row.get("folder_child_id");
            let path: String = row.get("path");
            let song_id: Uuid = row.get("song_id");
            (id, PathBuf::from_str(&path).unwrap(), song_id)
        })
        .fetch_all(conn.deref_mut())
        .await?;

        for (child_id, child_path, song_id) in children {
            if tokio::fs::metadata(&child_path).await.ok().is_none() {
                info!("File was removed: {:?}", child_path.as_os_str());

                sqlx::query(
                    r#"
                            DELETE FROM folder_children WHERE folder_child_id = ?;
                            DELETE FROM folder_children_failed WHERE folder_child_id = ?;
                            DELETE FROM songs WHERE song_id = ?;
                        "#,
                )
                .bind(child_id)
                .bind(song_id)
                .bind(song_id)
                .execute(conn.deref_mut())
                .await?;
            }
        }

        if folder_deleted {
            sqlx::query("DELETE FROM folders WHERE folder_id = ?")
                .bind(folder_id)
                .execute(conn.deref_mut())
                .await?;
        }
    }

    // Cleanup albums and artists without songs
    sqlx::query(
        r#"
        DELETE FROM album_artists
        WHERE (album_id, artist_id) IN
        (SELECT a.album_id, a.artist_id FROM album_artists a LEFT JOIN songs s on a.album_id = s.album_id AND a.artist_id = s.artist_id WHERE s.album_id IS NULL AND s.artist_id IS NULL);

        DELETE FROM albums
        WHERE album_id IN
        (SELECT a.album_id FROM albums a LEFT JOIN songs s on a.album_id = s.album_id WHERE s.album_id IS NULL);

        DELETE FROM artists
        WHERE artist_id IN
        (SELECT a.artist_id FROM artists a LEFT JOIN songs s on a.artist_id = s.artist_id WHERE s.artist_id IS NULL);
    "#,
    )
    .execute(conn.deref_mut())
    .await?;

    // Cleanup cover art and favorites
    sqlx::query(
        r#"
        DELETE FROM cover_art
        WHERE cover_art_id NOT IN (select song_id from songs UNION ALL select artist_id from songs UNION ALL select album_id from songs);

        DELETE FROM starred
        WHERE starred_id NOT IN (select song_id from songs UNION ALL select artist_id from songs UNION ALL select album_id from songs);
    "#,
    )
    .execute(conn.deref_mut())
    .await?;

    Ok(())
}
