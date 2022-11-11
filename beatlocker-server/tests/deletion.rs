use crate::test_utils::{copy_recursively, TestClient};
use axum::http::StatusCode;
use beatlocker_server::*;
use chrono::{DateTime, Utc};
use std::fs::{create_dir_all, remove_dir_all};
use std::path::PathBuf;
use std::sync::Arc;

#[path = "test_utils/mod.rs"]
mod test_utils;

use test_utils::*;

struct DirDeleter(PathBuf);

impl Drop for DirDeleter {
    fn drop(&mut self) {
        remove_dir_all(&self.0).unwrap();
    }
}

#[tokio::test]
async fn deletion_test() -> AppResult<()> {
    let temp_path = PathBuf::from("./deletion_test_data");
    create_dir_all(&temp_path)?;
    let _deleter = DirDeleter(temp_path.clone());
    copy_recursively("tests/data", &temp_path)?;

    enable_default_tracing();
    let options = ServerOptions {
        path: temp_path.clone(),
        database: DatabaseOptions {
            path: Some(PathBuf::from(".")),
            in_memory: true,
        },
        now_provider: Arc::new(Box::new(|| {
            DateTime::parse_from_rfc3339("2020-02-02T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        })),
        ..Default::default()
    };
    let app = App::new(options).await?;
    let client = TestClient::new(app.app.clone());

    app.task_manager
        .send(app.import_all_folders().await?)
        .await?;

    std::fs::remove_file(temp_path.join("Richard Bona/Richard Bona - Ba Senge.ogg"))?;

    app.task_manager
        .send(app.remove_deleted_files().await?)
        .await?;

    let res = client
        .get(&format!("/rest/getArtist?f=json&id={RICHARD_BONA_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!(
        "getArtistAfterDeletingBaSenge.json",
        res.json::<serde_json::Value>().await
    );

    remove_dir_all(temp_path.join("Richard Bona"))?;
    app.task_manager
        .send(app.remove_deleted_files().await?)
        .await?;

    let res = client.get("/rest/getAlbumList?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!(
        "getAlbumListAfterDeletingAlbum.json",
        res.json::<serde_json::Value>().await
    );
    let res = client.get("/rest/getArtists?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!(
        "getArtistsAfterDeletingAlbum.json",
        res.json::<serde_json::Value>().await
    );

    Ok(())
}
