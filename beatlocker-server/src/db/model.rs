use chrono::{DateTime, Utc};
use sqlx::types::Uuid;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DbFolder {
    pub folder_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub cover_art_id: Option<Uuid>,
    pub created: DateTime<Utc>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DbFolderChild {
    pub folder_child_id: Uuid,
    pub folder_id: Uuid,
    pub path: String,
    pub name: String,
    pub song_id: Option<Uuid>,
    pub last_updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DbFailedFolderChild {
    pub folder_child_id: Uuid,
    pub folder_id: Uuid,
    pub path: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DbAlbum {
    pub album_id: Uuid,
    pub title: String,
    pub cover_art_id: Option<Uuid>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DbArtist {
    pub artist_id: Uuid,
    pub name: String,
    pub cover_art_id: Option<Uuid>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DbSong {
    pub song_id: Uuid,
    pub title: String,
    pub created: DateTime<Utc>,
    pub date: Option<DateTime<Utc>>,
    pub cover_art_id: Option<Uuid>,
    pub artist_id: Option<Uuid>,
    pub album_id: Option<Uuid>,
    pub content_type: Option<String>,
    pub suffix: Option<String>,
    pub size: Option<u32>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub duration: Option<chrono::Duration>,
    pub bit_rate: Option<u32>,
    pub genre: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DbCoverArt {
    pub cover_art_id: Uuid,
    pub data: Vec<u8>,
}
