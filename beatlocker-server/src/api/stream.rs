use crate::{AppResult, SharedState};
use std::ops::DerefMut;

use axum::extract::{Query, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::body::AsyncReadBody;
use serde::Deserialize;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamParams {
    id: Uuid,
}

pub async fn stream(
    Query(params): Query<StreamParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

    let result = sqlx::query(
        "SELECT path, s.content_type FROM folder_children fc LEFT JOIN songs s ON s.song_id = fc.song_id WHERE folder_child_id = ?",
    )
    .bind(params.id)
    .map(|row: SqliteRow| {
        let path: String = row.get("path");
        let content_type: String = row.get("content_type");
        (path, content_type)
    })
    .fetch_optional(conn.deref_mut())
    .await?;

    match result {
        Some((path, content_type)) => {
            let file = tokio::fs::File::open(&path).await?;

            let headers = [
                (CONTENT_TYPE, &content_type),
                (CONTENT_LENGTH, &file.metadata().await?.len().to_string()),
            ];
            let body = AsyncReadBody::new(file);
            Ok((headers, body).into_response())
        }
        None => Ok((StatusCode::NOT_FOUND, ()).into_response()),
    }
}
