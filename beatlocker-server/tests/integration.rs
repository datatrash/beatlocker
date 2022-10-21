use crate::test_utils::TestClient;
use axum::http::header::CONTENT_TYPE;
use axum::http::StatusCode;
use beatlocker_server::*;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Arc;

const MOTORWAY_OST_FOLDER_UUID: &str = "68f8b71b-d9b4-c77e-c7f1-e4af263bcd93";
const MOTORWAY_OST_ALBUM_UUID: &str = "20d02390-2687-c407-2d28-d74f9fc6d5a1";
const MOTORWAY_OST_RADAR_UNIT_FOLDER_CHILD_UUID: &str = "9fe0fb24-dabd-4464-258b-1ab72a28aa94";
const MOTORWAY_OST_RADAR_UNIT_SONG_UUID: &str = "f417c310-98e2-e42f-ed0d-f9208c48419b";
const RICHARD_BONA_UUID: &str = "d094e9f8-a8e2-1737-0cfc-c4b24ab0aedf";

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
    let app = App::new(options).await?;
    let client = TestClient::new(app.app.clone());

    app.task_manager.send(app.import_all_folders()?).await?;

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

    let res = client
        .get(&format!(
            "/rest/getMusicDirectory?f=json&id={MOTORWAY_OST_FOLDER_UUID}"
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
            "/rest/getMusicDirectory?id={MOTORWAY_OST_FOLDER_UUID}"
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

    let res = client.get("/rest/getArtists?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getArtists.json", res.json::<serde_json::Value>().await);
    let res = client.get("/rest/getArtists.view").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getArtists.xml", res.xml_string().await);

    let res = client
        .get(&format!(
            "/rest/getArtistInfo?id={RICHARD_BONA_UUID}&f=json"
        ))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getArtistInfo.json", res.json::<serde_json::Value>().await);
    let res = client
        .get(&format!("/rest/getArtistInfo.view?id={RICHARD_BONA_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getArtistInfo.xml", res.xml_string().await);
    let res = client
        .get(&format!(
            "/rest/getArtistInfo2?id={RICHARD_BONA_UUID}&f=json"
        ))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getArtistInfo2.json", res.json::<serde_json::Value>().await);
    let res = client
        .get(&format!("/rest/getArtistInfo2.view?id={RICHARD_BONA_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getArtistInfo2.xml", res.xml_string().await);

    let res = client.get("/rest/getAlbumList?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getAlbumList.json", res.json::<serde_json::Value>().await);
    let res = client.get("/rest/getAlbumList.view").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getAlbumList.xml", res.xml_string().await);

    let res = client
        .get("/rest/getAlbumList2?f=json&type=alphabeticalByName")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getAlbumList2.json", res.json::<serde_json::Value>().await);
    let res = client
        .get("/rest/getAlbumList2.view?type=alphabeticalByName")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getAlbumList2.xml", res.xml_string().await);

    let res = client
        .get(&format!(
            "/rest/getAlbum?f=json&id={MOTORWAY_OST_ALBUM_UUID}"
        ))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getAlbum.json", res.json::<serde_json::Value>().await);
    let res = client
        .get(&format!("/rest/getAlbum?id={MOTORWAY_OST_ALBUM_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getAlbum.xml", res.xml_string().await);

    let res = client
        .get(&format!("/rest/getArtist?f=json&id={RICHARD_BONA_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getArtist.json", res.json::<serde_json::Value>().await);
    let res = client
        .get(&format!("/rest/getArtist?id={RICHARD_BONA_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getArtist.xml", res.xml_string().await);

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

    let res = client.get("/rest/getGenres?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getGenres.json", res.json::<serde_json::Value>().await);
    let res = client.get("/rest/getGenres").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getGenres.xml", res.xml_string().await);

    let res = client
        .get("/rest/getSongsByGenre?f=json&genre=World Music")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!(
        "getSongsByGenre.json",
        res.json::<serde_json::Value>().await
    );
    let res = client
        .get("/rest/getSongsByGenre?genre=World Music")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getSongsByGenre.xml", res.xml_string().await);

    let res = client.get("/rest/getPlaylists?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getPlaylists.json", res.json::<serde_json::Value>().await);
    let res = client.get("/rest/getPlaylists").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getPlaylists.xml", res.xml_string().await);

    let res = client
        .get(&format!(
            "/rest/getPlaylist?f=json&id={MOTORWAY_OST_FOLDER_UUID}"
        ))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getPlaylist.json", res.json::<serde_json::Value>().await);
    let res = client
        .get(&format!("/rest/getPlaylist?id={MOTORWAY_OST_FOLDER_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getPlaylist.xml", res.xml_string().await);

    // Try streaming
    let res = client
        .get(&"/rest/stream?id=1568a84c-22cd-2176-ab86-c69194a9de16".to_string())
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get(CONTENT_TYPE).unwrap().to_str().unwrap(),
        "audio/ogg"
    );
    assert_eq!(
        &res.bytes().await.to_vec(),
        include_bytes!("data/Richard Bona/Richard Bona - Ba Senge.ogg")
    );

    // Try get (non-existent) coverart
    let res = client
        .get(&"/rest/getCoverArt?id=1568a84c-22cd-2176-ab86-c69194a9de16".to_string())
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get(CONTENT_TYPE).unwrap().to_str().unwrap(),
        "image/jpeg"
    );
    assert_eq!(
        &res.bytes().await.to_vec(),
        include_bytes!("../src/api/fallback_cover.jpg")
    );

    let res = client
        .get(&format!("/rest/star?id={RICHARD_BONA_UUID}&id={MOTORWAY_OST_FOLDER_UUID}&id={MOTORWAY_OST_RADAR_UNIT_FOLDER_CHILD_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = client.get("/rest/getStarred?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getStarred.json", res.json::<serde_json::Value>().await);
    let res = client.get("/rest/getStarred").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getStarred.xml", res.xml_string().await);
    let res = client
        .get(&format!(
            "/rest/unstar?id={MOTORWAY_OST_RADAR_UNIT_SONG_UUID}"
        ))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = client.get("/rest/getStarred?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!(
        "getStarred_withoutRadarUnit.json",
        res.json::<serde_json::Value>().await
    );
    let res = client
        .get(&format!("/rest/unstar?id={RICHARD_BONA_UUID}&id={MOTORWAY_OST_FOLDER_UUID}&id={MOTORWAY_OST_RADAR_UNIT_FOLDER_CHILD_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = client
        .get(&format!("/rest/star?id={RICHARD_BONA_UUID}&id={MOTORWAY_OST_ALBUM_UUID}&id={MOTORWAY_OST_RADAR_UNIT_FOLDER_CHILD_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = client.get("/rest/getStarred2?f=json").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("getStarred2.json", res.json::<serde_json::Value>().await);
    let res = client.get("/rest/getStarred2").send().await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_snapshot!("getStarred2.xml", res.xml_string().await);

    println!("UNSTARRRRRR");
    let res = client
        .get(&format!("/rest/unstar?id={RICHARD_BONA_UUID}&id={MOTORWAY_OST_ALBUM_UUID}&id={MOTORWAY_OST_RADAR_UNIT_FOLDER_CHILD_UUID}"))
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);

    // Import everything again and see if there are no duplicates and nothing starred etc
    app.task_manager.send(app.import_all_folders()?).await?;
    let res = client
        .get("/rest/search3?f=json&query=\"\"&songCount=2")
        .send()
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    insta::assert_json_snapshot!("search3.json", res.json::<serde_json::Value>().await);

    Ok(())
}
