use crate::api::model::SubsonicSong;
use crate::AppResult;
use chrono::{Datelike, NaiveDateTime};

use sqlx::{Row, SqliteConnection};

use uuid::Uuid;

pub struct GetSubsonicSongsQuery {
    pub folder_id: Option<Uuid>,
    pub song_offset: u32,
    pub song_count: u32,
}

impl Default for GetSubsonicSongsQuery {
    fn default() -> Self {
        Self {
            folder_id: None,
            song_count: 20,
            song_offset: 0,
        }
    }
}

pub async fn get_subsonic_songs(
    conn: &mut SqliteConnection,
    query: GetSubsonicSongsQuery,
) -> AppResult<Vec<SubsonicSong>> {
    let where_clause = match query.folder_id {
        Some(_id) => "WHERE folder_id = ?",
        None => "",
    };
    let sql = format!(
        r#"SELECT fc.*, s.*, ar.name as artist, al.title as album FROM folder_children fc
        LEFT JOIN songs s ON s.song_id = fc.song_id
        LEFT JOIN artists ar ON ar.artist_id = s.artist_id
        LEFT JOIN albums al ON al.album_id = s.album_id
        {}
        ORDER BY s.title LIMIT ?, ?
        "#,
        where_clause
    );
    let query_builder = sqlx::query(&sql);
    let query_builder = if !where_clause.is_empty() {
        query_builder.bind(query.folder_id.unwrap())
    } else {
        query_builder
    };
    let songs = query_builder
        .bind(query.song_offset)
        .bind(query.song_count)
        .map(|row| {
            let id: Uuid = row.get("folder_child_id");
            let folder_id: Uuid = row.get("folder_id");
            let date: Option<NaiveDateTime> = row.get("date");
            let genre: Option<String> = row.get("genre");
            SubsonicSong {
                id,
                is_dir: false,
                parent: folder_id,
                title: row.get("name"),
                created: row.get("created"),
                cover_art: row.get("cover_art_id"),
                artist_id: row.get("artist_id"),
                artist: row.get("artist"),
                album_id: row.get("album_id"),
                album: row.get("album"),
                content_type: row.get("content_type"),
                suffix: row.get("suffix"),
                size: row.get("size"),
                track: row.get("track_number"),
                disc_number: row.get("disc_number"),
                duration: row.get("duration"),
                bit_rate: row.get("bit_rate"),
                year: date.map(|d| d.year() as u32),
                genre: Some(genre.unwrap_or_else(|| "Unknown genre".to_string())),
                ..Default::default()
            }
        })
        .fetch_all(conn)
        .await
        .unwrap();

    Ok(songs)
}
