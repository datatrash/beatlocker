use crate::{AppResult, TaskState};
use std::ops::DerefMut;
use std::sync::Arc;
use tracing::debug;

pub async fn optimize_database(state: Arc<TaskState>) -> AppResult<()> {
    debug!("Vacuuming database");
    sqlx::query("VACUUM")
        .execute(state.db.conn().await?.deref_mut())
        .await?;

    debug!("Truncating write-ahead log");
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(state.db.conn().await?.deref_mut())
        .await?;

    debug!("Database optimization complete");
    Ok(())
}
