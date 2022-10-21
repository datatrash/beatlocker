use crate::{AppResult, AppState};
use axum::extract::{Query, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use std::ops::DerefMut;

use serde::Deserialize;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCoverArtParams {
    id: Uuid,
}

pub async fn get_cover_art(
    Query(params): Query<GetCoverArtParams>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

    let data = sqlx::query("SELECT * FROM cover_art WHERE cover_art_id = ?")
        .bind(params.id)
        .map(|row: SqliteRow| {
            let data: Vec<u8> = row.get("data");
            data
        })
        .fetch_optional(conn.deref_mut())
        .await?;

    let data = data.unwrap_or_else(|| include_bytes!("fallback_cover.jpg").to_vec());

    let content_type = infer::get(&data)
        .map(|ty| ty.mime_type())
        .unwrap_or("application/octet-stream");
    let headers = [
        (CONTENT_TYPE, content_type),
        (CONTENT_LENGTH, &data.len().to_string()),
    ];
    Ok((headers, data).into_response())
}
