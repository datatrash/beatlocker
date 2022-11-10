use std::ops::DerefMut;
use std::str::FromStr;

use axum::extract::State;
use axum::response::Response;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

use crate::api::format::SubsonicFormat;
use crate::{AppResult, Deserialize, SharedState};

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StarParams {
    #[serde(default = "Vec::new")]
    id: Vec<String>,
    #[serde(default = "Vec::new")]
    album_id: Vec<String>,
    #[serde(default = "Vec::new")]
    artist_id: Vec<String>,
}

impl StarParams {
    pub fn all_ids(&self) -> Vec<Uuid> {
        let mut ids = vec![];
        ids.extend(self.id.iter().filter_map(|s| Uuid::from_str(s).ok()));
        ids.extend(self.album_id.iter().filter_map(|s| Uuid::from_str(s).ok()));
        ids.extend(self.artist_id.iter().filter_map(|s| Uuid::from_str(s).ok()));
        ids
    }
}

pub async fn star(
    format: SubsonicFormat,
    params: axum_extra::extract::Query<StarParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    let ids = params.all_ids();
    for id in ids {
        sqlx::query("INSERT OR IGNORE INTO starred (starred_id, created) VALUES (?, ?)")
            .bind(id)
            .bind((state.options.now_provider)())
            .execute(state.db.conn().await?.deref_mut())
            .await?;
    }

    Ok(format.render::<()>(None))
}

pub async fn unstar(
    format: SubsonicFormat,
    params: axum_extra::extract::Query<StarParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    let ids = params.all_ids();
    for id in ids {
        // This is absolutely terrible, but since items can be starred based on both the song_id
        // or the folder_child_id (or album/artist_id) we have to do this extra query
        let folder_child_id = sqlx::query(
            r#"
                SELECT folder_child_id
                FROM folder_children
                WHERE folder_child_id = ? OR song_id = ?"#,
        )
        .bind(id)
        .bind(id)
        .map(|row: SqliteRow| {
            let folder_child_id: Uuid = row.get("folder_child_id");
            folder_child_id
        })
        .fetch_optional(state.db.conn().await?.deref_mut())
        .await
        .unwrap();

        for id in [Some(id), folder_child_id].iter().flatten() {
            sqlx::query("DELETE FROM starred WHERE starred_id = ?")
                .bind(id)
                .execute(state.db.conn().await?.deref_mut())
                .await?
                .rows_affected();
        }
    }

    Ok(format.render::<()>(None))
}
