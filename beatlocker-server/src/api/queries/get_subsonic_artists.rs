use crate::api::model::SubsonicArtist;
use crate::AppResult;

use sqlx::{Row, SqliteConnection};
use uuid::Uuid;

pub struct GetSubsonicArtistsQuery {
    pub folder_id: Option<u32>,
    pub artist_offset: u32,
    pub artist_count: u32,
}

impl Default for GetSubsonicArtistsQuery {
    fn default() -> Self {
        Self {
            folder_id: None,
            artist_count: 20,
            artist_offset: 0,
        }
    }
}

pub async fn get_subsonic_artists(
    conn: &mut SqliteConnection,
    query: GetSubsonicArtistsQuery,
) -> AppResult<Vec<SubsonicArtist>> {
    let where_clause = match query.folder_id {
        Some(_id) => "WHERE folder_id = ?",
        None => "",
    };
    let sql = format!(
        r#"SELECT artists.*, COUNT(aa.album_id) as album_count FROM artists
        LEFT JOIN album_artists aa on artists.artist_id = aa.artist_id
        {}
        GROUP BY 1
        ORDER BY artist_id LIMIT ?, ?
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
        .bind(query.artist_offset)
        .bind(query.artist_count)
        .map(|row| {
            let id: Uuid = row.get("artist_id");
            SubsonicArtist {
                id,
                name: row.get("name"),
                cover_art: row.get("cover_art_id"),
                album_count: row.get("album_count"),
            }
        })
        .fetch_all(conn)
        .await
        .unwrap();

    Ok(artists)
}
