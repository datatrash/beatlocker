use std::ops::DerefMut;
use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::{SubsonicChild, SubsonicChildDirectory};
use crate::api::queries::{get_subsonic_songs, GetSubsonicSongsQuery};
use crate::{AppResult, AppState};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMusicDirectoryParams {
    id: Uuid,
}

pub async fn get_music_directory(
    format: SubsonicFormat,
    Query(params): Query<GetMusicDirectoryParams>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

    let parent_name = sqlx::query("SELECT * FROM folders WHERE folder_id = ?")
        .bind(params.id)
        .map(|row: SqliteRow| {
            let name: String = row.get("name");
            name
        })
        .fetch_optional(conn.deref_mut())
        .await?;

    match parent_name {
        Some(parent_name) => {
            let folders = sqlx::query("SELECT * FROM folders WHERE parent_id = ?")
                .bind(params.id)
                .map(|row: SqliteRow| {
                    let id: Uuid = row.get("folder_id");
                    SubsonicChild::ChildDirectory(SubsonicChildDirectory {
                        id,
                        parent: params.id,
                        is_dir: true,
                        title: row.get("name"),
                        name: row.get("name"),
                        created: row.get("created"),
                        ..Default::default()
                    })
                })
                .fetch_all(conn.deref_mut())
                .await?;

            let children = get_subsonic_songs(
                conn.deref_mut(),
                GetSubsonicSongsQuery {
                    folder_id: Some(params.id),
                    ..Default::default()
                },
            )
            .await?
            .into_iter()
            .map(SubsonicChild::ChildSong)
            .collect_vec();
            let results = [&folders[..], &children[..]].concat();

            Ok(format.render(GetMusicDirectoryResponse {
                directory: Directory {
                    id: params.id,
                    name: parent_name,
                    child: results,
                },
            }))
        }
        None => Ok((StatusCode::NOT_FOUND, ()).into_response()),
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMusicDirectoryResponse {
    //last_modified: String,
    directory: Directory,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename = "directory", rename_all = "camelCase")]
pub struct Directory {
    id: Uuid,
    name: String,
    child: Vec<SubsonicChild>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlGetMusicDirectoryResponse {
    #[serde(rename_all = "camelCase")]
    Directory {
        id: Uuid,
        name: String,
        child: Vec<SubsonicChild>,
    },
}

impl ToXml for GetMusicDirectoryResponse {
    type Output = XmlGetMusicDirectoryResponse;

    fn into_xml(self) -> Self::Output {
        XmlGetMusicDirectoryResponse::Directory {
            id: self.directory.id,
            name: self.directory.name,
            child: self.directory.child,
        }
    }
}
