use std::ops::DerefMut;
use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::SubsonicSong;
use crate::api::queries::{get_subsonic_songs, GetSubsonicSongsQuery};
use crate::{AppResult, AppState, Deserialize, Serialize};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPlaylistParams {
    id: Uuid,
}

pub async fn get_playlist(
    format: SubsonicFormat,
    Query(params): Query<GetPlaylistParams>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

    let playlist = sqlx::query(
        r#"
        SELECT f.*, COUNT(fc.song_id) AS song_count, SUM(s.duration) AS duration
        FROM folders f
        LEFT JOIN folder_children fc on f.folder_id = fc.folder_id
        LEFT JOIN songs s on fc.song_id = s.song_id
        WHERE f.folder_id = ?
        GROUP BY 1
    "#,
    )
    .bind(params.id)
    .map(|row: SqliteRow| {
        let id: Uuid = row.get("folder_id");
        Playlist {
            id,
            name: row.get("name"),
            created: row.get("created"),
            public: true,
            song_count: row.get("song_count"),
            duration: row.get("duration"),
            cover_art: row.get("cover_art_id"),
            entry: vec![],
        }
    })
    .fetch_optional(conn.deref_mut())
    .await?;

    match playlist {
        Some(mut playlist) => {
            playlist.entry = get_subsonic_songs(
                &mut conn,
                GetSubsonicSongsQuery {
                    folder_id: Some(params.id),
                    song_count: 50000,
                    ..Default::default()
                },
            )
            .await?;

            Ok(format.render(GetPlaylistResponse { playlist }))
        }
        None => Ok((StatusCode::NOT_FOUND, ()).into_response()),
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPlaylistResponse {
    playlist: Playlist,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    id: Uuid,
    name: String,
    created: DateTime<Utc>,
    public: bool,
    song_count: u32,
    duration: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_art: Option<Uuid>,
    entry: Vec<SubsonicSong>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlGetPlaylistResponse {
    Playlist {
        id: Uuid,
        name: String,
        created: DateTime<Utc>,
        public: bool,
        song_count: u32,
        duration: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        cover_art: Option<Uuid>,
        entry: Vec<SubsonicSong>,
    },
}

impl ToXml for GetPlaylistResponse {
    type Output = XmlGetPlaylistResponse;

    fn into_xml(self) -> Self::Output {
        XmlGetPlaylistResponse::Playlist {
            id: self.playlist.id,
            name: self.playlist.name,
            created: self.playlist.created,
            public: self.playlist.public,
            song_count: self.playlist.song_count,
            duration: self.playlist.duration,
            cover_art: self.playlist.cover_art,
            entry: self.playlist.entry,
        }
    }
}
