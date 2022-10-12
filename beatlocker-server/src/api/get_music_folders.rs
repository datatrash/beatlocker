use crate::api::format::{SubsonicFormat, ToXml};
use crate::AppResult;

use axum::response::Response;
use serde::{Deserialize, Serialize};

use uuid::Uuid;

pub async fn get_music_folders(format: SubsonicFormat) -> AppResult<Response> {
    Ok(format.render(MusicFoldersResponse {
        music_folders: MusicFolders {
            music_folder: vec![MusicFolder {
                id: Uuid::nil(),
                name: "Music".to_owned(),
            }],
        },
    }))
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFoldersResponse {
    music_folders: MusicFolders,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFolders {
    music_folder: Vec<MusicFolder>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlMusicFoldersResponse {
    #[serde(rename_all = "camelCase")]
    MusicFolders(Vec<MusicFolder>),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename = "musicFolder", rename_all = "camelCase")]
pub struct MusicFolder {
    pub id: Uuid,
    pub name: String,
}
impl ToXml for MusicFoldersResponse {
    type Output = XmlMusicFoldersResponse;

    fn into_xml(self) -> Self::Output {
        XmlMusicFoldersResponse::MusicFolders(self.music_folders.music_folder)
    }
}
