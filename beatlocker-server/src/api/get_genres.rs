use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::UNKNOWN_GENRE;
use crate::{AppResult, Deserialize, Serialize, SharedState};
use axum::extract::State;
use axum::response::Response;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use std::collections::BTreeMap;
use std::ops::DerefMut;

pub async fn get_genres(
    format: SubsonicFormat,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    let genre_songs: Vec<(String, u32)> = sqlx::query(
        r#"
    SELECT
        count(s.song_id) AS song_count,
        s.genre
    FROM songs s
    GROUP BY s.genre
    "#,
    )
    .map(|row: SqliteRow| {
        let genre: Option<String> = row.get("genre");
        (
            genre.unwrap_or_else(|| UNKNOWN_GENRE.to_string()),
            row.get("song_count"),
        )
    })
    .fetch_all(state.db.conn().await?.deref_mut())
    .await?;

    let genre_albums: Vec<(String, u32)> = sqlx::query(
        r#"
        SELECT count(a.album_id) AS album_count, s.genre
        FROM ALBUMS a
        LEFT JOIN songs s on a.album_id = s.album_id
        GROUP BY s.genre
    "#,
    )
    .map(|row: SqliteRow| {
        let genre: Option<String> = row.get("genre");
        (
            genre.unwrap_or_else(|| UNKNOWN_GENRE.to_string()),
            row.get("album_count"),
        )
    })
    .fetch_all(state.db.conn().await?.deref_mut())
    .await?;

    let genre_songs: BTreeMap<String, u32> = genre_songs.into_iter().collect();
    let genre_albums: BTreeMap<String, u32> = genre_albums.into_iter().collect();

    let mut result: BTreeMap<String, Genre> = BTreeMap::new();
    for (value, song_count) in genre_songs {
        let e = result.entry(value.clone()).or_insert(Genre {
            song_count,
            album_count: 0,
            value: value.clone(),
        });
        e.album_count = genre_albums.get(&value).cloned().unwrap_or_default();
    }

    Ok(format.render(GetGenresResponse {
        genres: Genres {
            genre: result.into_values().collect(),
        },
    }))
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGenresResponse {
    genres: Genres,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Genres {
    genre: Vec<Genre>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    song_count: u32,
    album_count: u32,
    value: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlGetGenresResponse {
    #[serde(rename_all = "camelCase")]
    Genres(Vec<XmlGenre>),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename = "genre", rename_all = "camelCase")]
pub struct XmlGenre {
    song_count: u32,
    album_count: u32,
    #[serde(rename = "$value")]
    value: String,
}

impl ToXml for GetGenresResponse {
    type Output = XmlGetGenresResponse;

    fn into_xml(self) -> Self::Output {
        XmlGetGenresResponse::Genres(
            self.genres
                .genre
                .into_iter()
                .map(|g| XmlGenre {
                    song_count: g.song_count,
                    album_count: g.album_count,
                    value: g.value,
                })
                .collect(),
        )
    }
}
