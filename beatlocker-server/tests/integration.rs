use crate::test_utils::TestClient;
use axum::http::StatusCode;
use beatlocker_server::*;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Arc;

#[path = "test_utils/mod.rs"]
mod test_utils;

#[tokio::test]
async fn integration_test() -> AppResult<()> {
    enable_default_tracing();
    let options = ServerOptions {
        path: PathBuf::from("tests/data"),
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
    let server = App::new(options).await?;
    let client = TestClient::new(server.app.clone());

    server.import_all_folders().await?;

    let res = client.get("/rest/ping?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("ping.json", res.json::<serde_json::Value>().await);
    let res = client.get("/rest/ping").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("ping.xml", res.xml_string().await);

    let res = client.get("/rest/getLicense?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getLicense.json", res.json::<serde_json::Value>().await);
    let res = client.get("/rest/getLicense").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getLicense.xml", res.xml_string().await);

    let res = client.get("/rest/getMusicFolders?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!(
        "getMusicFolders.json",
        res.json::<serde_json::Value>().await
    );
    let res = client.get("/rest/getMusicFolders").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getMusicFolders.xml", res.xml_string().await);

    let motorway_ost_folder_uuid = "9600e0e0-145e-644d-d38f-e501a6252d79";
    let res = client
        .get(&format!(
            "/rest/getMusicDirectory?f=json&id={motorway_ost_folder_uuid}"
        ))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!(
        "getMusicDirectory.json",
        res.json::<serde_json::Value>().await
    );
    let res = client
        .get(&format!(
            "/rest/getMusicDirectory?id={motorway_ost_folder_uuid}"
        ))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getMusicDirectory.xml", res.xml_string().await);

    let res = client
        .get("/rest/getIndexes?f=json&musicFolderId=00000000-0000-0000-0000-000000000000")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getIndexes.json", res.json::<serde_json::Value>().await);
    let res = client
        .get("/rest/getIndexes?musicFolderId=00000000-0000-0000-0000-000000000000")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getIndexes.xml", res.xml_string().await);

    let res = client
        .get("/rest/getAlbumList?f=json&type=alphabeticalByName")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getAlbumList.json", res.json::<serde_json::Value>().await);
    let res = client
        .get("/rest/getAlbumList.view?type=alphabeticalByName")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getAlbumList.xml", res.xml_string().await);

    let res = client
        .get("/rest/search3?f=json&query=\"\"&songCount=2")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("search3.json", res.json::<serde_json::Value>().await);
    let res = client
        .get("/rest/search3?query=\"\"&songCount=2")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("search3.xml", res.xml_string().await);

    let res = client.get("/rest/getPlaylists?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getPlaylists.json", res.json::<serde_json::Value>().await);
    let res = client.get("/rest/getPlaylists").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getPlaylists.xml", res.xml_string().await);

    let res = client
        .get(&format!(
            "/rest/getPlaylist?f=json&id={motorway_ost_folder_uuid}"
        ))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getPlaylist.json", res.json::<serde_json::Value>().await);
    let res = client
        .get(&format!("/rest/getPlaylist?id={motorway_ost_folder_uuid}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getPlaylist.xml", res.xml_string().await);

    Ok(())
}
