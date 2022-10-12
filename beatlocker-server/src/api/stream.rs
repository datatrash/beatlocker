use std::ops::DerefMut;
use crate::api::format::SubsonicFormat;
use crate::{AppResult, AppState};

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
    _format: SubsonicFormat,
    Query(params): Query<StreamParams>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

    let path = sqlx::query("SELECT path FROM folder_children WHERE folder_child_id = ?")
        .bind(params.id)
        .map(|row: SqliteRow| {
            let path: String = row.get("path");
            path
        })
        .fetch_optional(conn.deref_mut())
        .await?;

    match path {
        Some(path) => {
            let file = tokio::fs::File::open(&path).await?;

            let headers = [
                (CONTENT_TYPE, "audio/ogg"),
                (CONTENT_LENGTH, &file.metadata().await?.len().to_string()),
            ];
            let body = AsyncReadBody::new(file);
            Ok((headers, body).into_response())
        }
        None => Ok((StatusCode::NOT_FOUND, ()).into_response()),
    }
}
