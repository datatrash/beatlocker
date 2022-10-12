use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::SubsonicAlbum;
use crate::api::queries::{get_subsonic_albums, GetSubsonicAlbumsQuery};
use crate::{AppResult, AppState};
use axum::extract::{Query, State};
use axum::response::Response;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAlbumListParams {
    size: Option<u32>,
    offset: Option<u32>,
}

pub async fn get_album_list(
    format: SubsonicFormat,
    Query(params): Query<GetAlbumListParams>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

    let results = get_subsonic_albums(
        &mut conn,
        GetSubsonicAlbumsQuery {
            album_offset: params.offset.unwrap_or_default(),
            album_count: params.size.unwrap_or(10),
            ..Default::default()
        },
    )
    .await?;

    Ok(format.render(AlbumListResponse {
        album_list: { AlbumList { albums: results } },
    }))
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumListResponse {
    album_list: AlbumList,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumList {
    albums: Vec<SubsonicAlbum>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlAlbumListResponse {
    #[serde(rename_all = "camelCase")]
    AlbumList(Vec<SubsonicAlbum>),
}

impl ToXml for AlbumListResponse {
    type Output = XmlAlbumListResponse;

    fn into_xml(self) -> Self::Output {
        XmlAlbumListResponse::AlbumList(
            self.album_list
                .albums
                .into_iter()
                .map(|a| SubsonicAlbum {
                    id: a.id,
                    parent: a.parent,
                    is_dir: a.is_dir,
                    name: a.title.clone(),
                    title: a.title,
                    song_count: a.song_count,
                    duration: a.duration,
                    artist: a.artist,
                    artist_id: a.artist_id,
                    cover_art: a.cover_art,
                })
                .collect(),
        )
    }
}
