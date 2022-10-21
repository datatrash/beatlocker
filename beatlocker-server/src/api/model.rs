use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const UNKNOWN_GENRE: &str = "[Unknown genre]";

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename = "song", rename_all = "camelCase")]
pub struct SubsonicSong {
    pub id: Uuid,
    pub parent: Uuid,
    pub is_dir: bool,
    pub created: DateTime<Utc>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<Uuid>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    pub is_video: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starred: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename = "artist", rename_all = "camelCase")]
pub struct SubsonicArtist {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<Uuid>,
    pub album_count: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub album: Vec<SubsonicAlbum>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub song: Vec<SubsonicSong>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starred: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename = "album", rename_all = "camelCase")]
pub struct SubsonicAlbum {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dir: Option<bool>,
    pub name: String,
    pub title: String,
    pub song_count: u32,
    pub duration: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<Uuid>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub song: Vec<SubsonicSong>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starred: Option<DateTime<Utc>>,
}

impl Default for SubsonicAlbum {
    fn default() -> Self {
        Self {
            id: Default::default(),
            parent: None,
            is_dir: None,
            name: "".to_string(),
            title: "".to_string(),
            song_count: 0,
            duration: 0,
            artist: None,
            artist_id: None,
            cover_art: None,
            song: vec![],
            starred: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged, rename = "child", rename_all = "camelCase")]
pub enum SubsonicChild {
    ChildDirectory(SubsonicChildDirectory),
    ChildSong(SubsonicSong),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename = "child", rename_all = "camelCase")]
pub struct SubsonicChildDirectory {
    pub id: Uuid,
    pub parent: Uuid,
    pub is_dir: bool,
    pub title: String,
    pub name: String,
    pub created: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub song_count: Option<usize>,
    pub is_video: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct XmlStringWrapper(pub String);
