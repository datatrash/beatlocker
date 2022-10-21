use crate::api::model::SubsonicArtist;
use crate::AppResult;

use sqlx::{QueryBuilder, Row, SqliteConnection};
use uuid::Uuid;

pub struct GetSubsonicArtistsQuery {
    pub artist_id: Option<Uuid>,
    pub folder_id: Option<Uuid>,
    pub artist_offset: u32,
    pub artist_count: u32,
    pub starred: bool,
}

impl Default for GetSubsonicArtistsQuery {
    fn default() -> Self {
        Self {
            artist_id: None,
            folder_id: None,
            artist_count: 20,
            artist_offset: 0,
            starred: false,
        }
    }
}

pub async fn get_subsonic_artists(
    conn: &mut SqliteConnection,
    query: GetSubsonicArtistsQuery,
) -> AppResult<Vec<SubsonicArtist>> {
    let mut builder = QueryBuilder::new(
        "SELECT artists.*, COUNT(aa.album_id) as album_count, st.created as starred_date
        FROM artists
        LEFT JOIN album_artists aa on artists.artist_id = aa.artist_id
        LEFT JOIN starred st ON st.starred_id = artists.artist_id
        WHERE 1=1
        ",
    );

    if let Some(id) = query.artist_id {
        builder.push(" AND artists.artist_id = ").push_bind(id);
    }
    if let Some(id) = query.folder_id {
        builder.push(" AND folder_id = ").push_bind(id);
    }
    if query.starred {
        builder.push(" AND starred_date IS NOT NULL");
    }
    builder
        .push(" GROUP BY 1 ORDER BY artist_id LIMIT ")
        .push_bind(query.artist_offset)
        .push(", ")
        .push_bind(query.artist_count);
    let artists = builder
        .build()
        .map(|row| {
            let id: Uuid = row.get("artist_id");
            SubsonicArtist {
                id,
                name: row.get("name"),
                cover_art: row.get("cover_art_id"),
                album_count: row.get("album_count"),
                album: vec![],
                starred: row.get("starred_date"),
                song: vec![],
            }
        })
        .fetch_all(conn)
        .await
        .unwrap();

    Ok(artists)
}
