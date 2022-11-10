use crate::api::format::{SubsonicFormat, ToXml};
use crate::{AppResult, Deserialize, Serialize, SharedState};
use axum::extract::State;
use axum::response::Response;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use std::ops::DerefMut;
use uuid::Uuid;

pub async fn get_playlists(
    format: SubsonicFormat,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

    let results = sqlx::query(
        r#"
        SELECT f.*, COUNT(fc.song_id) AS song_count, SUM(s.duration) AS duration
        FROM folders f
        LEFT JOIN folder_children fc on f.folder_id = fc.folder_id
        LEFT JOIN songs s on fc.song_id = s.song_id
        WHERE parent_id IS NOT NULL
        GROUP BY 1
        ORDER BY f.name
    "#,
    )
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
        }
    })
    .fetch_all(conn.deref_mut())
    .await?;

    Ok(format.render(GetPlaylistsResponse {
        playlists: Playlists { playlist: results },
    }))
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPlaylistsResponse {
    playlists: Playlists,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlists {
    playlist: Vec<Playlist>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
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
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlGetPlaylistsResponse {
    #[serde(rename_all = "camelCase")]
    Playlists { playlist: Vec<Playlist> },
}

impl ToXml for GetPlaylistsResponse {
    type Output = XmlGetPlaylistsResponse;

    fn into_xml(self) -> Self::Output {
        XmlGetPlaylistsResponse::Playlists {
            playlist: self.playlists.playlist,
        }
    }
}
