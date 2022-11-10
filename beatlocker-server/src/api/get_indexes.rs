use crate::api::format::{SubsonicFormat, ToXml};
use crate::{AppResult, SharedState};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row};
use std::ops::DerefMut;

use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetIndexesParams {
    music_folder_id: Option<Uuid>,
}

pub async fn get_indexes(
    format: SubsonicFormat,
    Query(params): Query<GetIndexesParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

    let mut builder = QueryBuilder::new("SELECT * FROM folders");
    match params.music_folder_id {
        Some(id) => builder.push(" WHERE parent_id = ").push_bind(id),
        None => builder.push(" WHERE parent_id IS NOT NULL"),
    };
    let folders = builder
        .build()
        .map(|row: SqliteRow| {
            let id: Uuid = row.get("folder_id");
            IndexArtist {
                id: id.to_string(),
                name: row.get("name"),
                album_count: 1,
            }
        })
        .fetch_all(conn.deref_mut())
        .await?;

    if folders.is_empty() {
        return Ok((StatusCode::NOT_FOUND, ()).into_response());
    }

    let index = folders
        .into_iter()
        .group_by(|ia| ia.name.chars().next().unwrap_or_default())
        .into_iter()
        .map(|(index, artist)| Index {
            name: index.to_string(),
            artist: artist.collect_vec(),
        })
        .sorted_by_key(|index| index.name.clone())
        .collect();

    Ok(format.render(GetIndexesResponse {
        indexes: Indexes { index },
        ignored_articles: "The El La Los Las Le Les Os As O A".to_owned(),
    }))
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetIndexesResponse {
    indexes: Indexes,
    //last_modified: usize,
    ignored_articles: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Indexes {
    index: Vec<Index>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    name: String,
    artist: Vec<IndexArtist>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexArtist {
    id: String,
    name: String,
    album_count: usize,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlGetIndexesResponse {
    #[serde(rename_all = "camelCase")]
    Indexes {
        index: Vec<Index>,
        ignored_articles: String,
    },
}

impl ToXml for GetIndexesResponse {
    type Output = XmlGetIndexesResponse;

    fn into_xml(self) -> Self::Output {
        XmlGetIndexesResponse::Indexes {
            index: self.indexes.index,
            ignored_articles: self.ignored_articles,
        }
    }
}
