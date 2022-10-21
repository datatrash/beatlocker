use crate::api::model::{SubsonicSong, UNKNOWN_GENRE};
use crate::AppResult;
use chrono::{DateTime, Datelike, NaiveDateTime, Utc};

use sqlx::{QueryBuilder, Row, SqliteConnection};

use uuid::Uuid;

pub struct GetSubsonicSongsQuery {
    pub folder_id: Option<Uuid>,
    pub album_id: Option<Uuid>,
    pub artist_id: Option<Uuid>,
    pub genre: Option<String>,
    pub song_offset: u32,
    pub song_count: u32,
    pub starred: bool,
    pub from_year: Option<u32>,
    pub to_year: Option<u32>,
    pub random: bool,
}

impl Default for GetSubsonicSongsQuery {
    fn default() -> Self {
        Self {
            folder_id: None,
            album_id: None,
            artist_id: None,
            genre: None,
            song_count: 20,
            song_offset: 0,
            starred: false,
            from_year: None,
            to_year: None,
            random: false,
        }
    }
}

pub async fn get_subsonic_songs(
    conn: &mut SqliteConnection,
    query: GetSubsonicSongsQuery,
) -> AppResult<Vec<SubsonicSong>> {
    let mut builder = QueryBuilder::new(
        r#"SELECT fc.folder_child_id, fc.folder_id, s.*, ar.name as artist, al.title as album, st.created as starred_date
        FROM folder_children fc
        LEFT JOIN songs s ON s.song_id = fc.song_id
        LEFT JOIN artists ar ON ar.artist_id = s.artist_id
        LEFT JOIN albums al ON al.album_id = s.album_id
        LEFT JOIN starred st ON st.starred_id = s.song_id OR st.starred_id = fc.folder_child_id
        WHERE 1=1
        "#,
    );

    if let Some(id) = query.folder_id {
        builder.push(" AND folder_id = ").push_bind(id);
    };
    if let Some(id) = query.album_id {
        builder.push(" AND al.album_id = ").push_bind(id);
    };
    if let Some(id) = query.artist_id {
        builder.push(" AND s.artist_id = ").push_bind(id);
    };
    if let Some(id) = query.genre {
        match id.as_str() {
            id if id == UNKNOWN_GENRE => builder.push(" AND s.genre IS NULL"),
            _ => builder.push(" AND s.genre = ").push_bind(id),
        };
    };
    if query.starred {
        builder.push(" AND starred_date IS NOT NULL");
    }

    if let Some(year) = query.from_year {
        let year: DateTime<Utc> = DateTime::default().with_year(year as i32).unwrap();
        builder.push(" AND s.date >= ").push_bind(year);
    }
    if let Some(year) = query.to_year {
        let year: DateTime<Utc> = DateTime::default().with_year(year as i32).unwrap();
        builder.push(" AND s.date <= ").push_bind(year);
    }

    if query.random {
        builder.push(" ORDER BY RANDOM()");
    } else {
        builder.push(" ORDER BY s.title");
    }

    builder
        .push(" LIMIT ")
        .push_bind(query.song_offset)
        .push(", ")
        .push_bind(query.song_count);

    let songs = builder
        .build()
        .map(|row| {
            let id: Uuid = row.get("folder_child_id");
            let folder_id: Uuid = row.get("folder_id");
            let date: Option<NaiveDateTime> = row.get("date");
            let genre: Option<String> = row.get("genre");
            SubsonicSong {
                id,
                is_dir: false,
                parent: folder_id,
                title: row.get("title"),
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
                starred: row.get("starred_date"),
                ..Default::default()
            }
        })
        .fetch_all(conn)
        .await
        .unwrap();

    Ok(songs)
}
