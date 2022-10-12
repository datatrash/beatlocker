use crate::api::model::SubsonicAlbum;
use crate::AppResult;

use sqlx::pool::PoolConnection;

use sqlx::{Row, Sqlite};
use uuid::Uuid;

pub struct GetSubsonicAlbumsQuery {
    pub folder_id: Option<Uuid>,
    pub album_offset: u32,
    pub album_count: u32,
}

impl Default for GetSubsonicAlbumsQuery {
    fn default() -> Self {
        Self {
            folder_id: None,
            album_count: 20,
            album_offset: 0,
        }
    }
}

pub async fn get_subsonic_albums(
    conn: &mut PoolConnection<Sqlite>,
    query: GetSubsonicAlbumsQuery,
) -> AppResult<Vec<SubsonicAlbum>> {
    let where_clause = match query.folder_id {
        Some(_id) => "WHERE folder_id = ?",
        None => "",
    };
    let sql = format!(
        r#"SELECT albums.*, ar.name AS artist_name, ar.artist_id AS artist_id, COUNT(s.song_id) AS song_count, SUM(s.duration) AS duration
        FROM albums
        LEFT JOIN album_artists aa on albums.album_id = aa.album_id
        LEFT JOIN artists ar on aa.artist_id = ar.artist_id
        LEFT JOIN songs s on s.album_id = albums.album_id
        {}
        GROUP BY 1
        ORDER BY album_id LIMIT ?, ?
        "#,
        where_clause
    );
    let query_builder = sqlx::query(&sql);
    let query_builder = if !where_clause.is_empty() {
        query_builder.bind(query.folder_id.unwrap())
    } else {
        query_builder
    };
    let artists = query_builder
        .bind(query.album_offset)
        .bind(query.album_count)
        .map(|row| {
            let id: Uuid = row.get("album_id");
            SubsonicAlbum {
                id,
                parent: Uuid::nil(),
                is_dir: true,
                name: row.get("title"),
                title: row.get("title"),
                song_count: row.get("song_count"),
                duration: row.get("duration"),
                artist: row.get("artist_name"),
                artist_id: row.get("artist_id"),
                cover_art: row.get("cover_art_id"),
            }
        })
        .fetch_all(conn)
        .await
        .unwrap();

    Ok(artists)
}
