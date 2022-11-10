use crate::test_utils::{copy_recursively, TestClient};
use axum::http::StatusCode;
use beatlocker_server::*;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Arc;

#[path = "test_utils/mod.rs"]
mod test_utils;

use test_utils::*;

#[tokio::test]
async fn deletion_test() -> AppResult<()> {
    let tempdir = tempfile::Builder::new().rand_bytes(0).tempdir_in(".")?;
    copy_recursively("tests/data", tempdir.path())?;

    enable_default_tracing();
    let options = ServerOptions {
        path: tempdir.path().to_path_buf(),
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

    std::fs::remove_file(
        tempdir
            .path()
            .join("Richard Bona/Richard Bona - Ba Senge.ogg"),
    )?;

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

    std::fs::remove_dir_all(tempdir.path().join("Richard Bona"))?;
    app.task_manager
        .send(app.remove_deleted_files().await?)
        .await?;

    let res = client
        .get(&format!("/rest/getAlbumList?f=json"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!(
        "getAlbumListAfterDeletingAlbum.json",
        res.json::<serde_json::Value>().await
    );
    let res = client.get(&format!("/rest/getArtists?f=json")).send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!(
        "getArtistsAfterDeletingAlbum.json",
        res.json::<serde_json::Value>().await
    );

    Ok(())
}
