#![allow(dead_code, unused)]
use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::SubsonicAlbum;
use crate::api::queries::{
    get_subsonic_albums, get_subsonic_albums_by_id3, GetSubsonicAlbumsListType,
    GetSubsonicAlbumsQuery,
};
use crate::{AlbumList2Response, AppResult, AppState, Db, GetAlbumList2Params, SharedState};
use axum::extract::{Query, State};
use axum::response::Response;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAlbumListParams {
    //music_folder_id: Option<Uuid>,
    size: Option<u32>,
    offset: Option<u32>,
}

pub async fn get_album_list(
    format: SubsonicFormat,
    ty: GetSubsonicAlbumsListType,
    Query(params): Query<GetAlbumListParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    Ok(format.render(get_album_list_impl(&state.read().await.db, params, ty).await?))
}

async fn get_album_list_impl(
    db: &Db,
    params: GetAlbumListParams,
    ty: GetSubsonicAlbumsListType,
) -> AppResult<AlbumListResponse> {
    let mut conn = db.conn().await?;

    let results = get_subsonic_albums(
        &mut conn,
        GetSubsonicAlbumsQuery {
            //music_folder_id: params.music_folder_id,
            offset: params.offset.unwrap_or_default(),
            size: params.size.unwrap_or(10),
            ty,
            ..Default::default()
        },
    )
    .await?;

    Ok(AlbumListResponse {
        album_list: { AlbumList { album: results } },
    })
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumListResponse {
    album_list: AlbumList,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumList {
    album: Vec<SubsonicAlbum>,
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
                .album
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
                    song: vec![],
                    starred: a.starred,
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestState;
    use itertools::Itertools;
    use std::sync::Arc;

    #[tokio::test]
    async fn sort_alphabetical_by_name() {
        let state = TestState::new().await.unwrap();

        assert_eq!(
            get(
                state.db().await,
                GetSubsonicAlbumsListType::AlphabeticalByName
            )
            .await,
            &["folder1", "folder2", "folder3"]
        );
    }

    #[tokio::test]
    async fn sort_alphabetical_by_artist() {
        // Doesn't really work yet, but sorts by folder name instead
        let state = TestState::new().await.unwrap();

        assert_eq!(
            get(
                state.db().await,
                GetSubsonicAlbumsListType::AlphabeticalByArtist
            )
            .await,
            &["folder1", "folder2", "folder3"]
        );
    }

    #[tokio::test]
    async fn query_by_year() {
        let state = TestState::new().await.unwrap();

        assert_eq!(
            get(
                state.db().await,
                GetSubsonicAlbumsListType::ByYear {
                    from_year: 2014,
                    to_year: 2022
                }
            )
            .await,
            &["folder3", "folder1", "folder2"]
        );

        assert_eq!(
            get(
                state.db().await,
                GetSubsonicAlbumsListType::ByYear {
                    from_year: 2022,
                    to_year: 2014
                }
            )
            .await,
            &["folder1", "folder2", "folder3"]
        );
    }

    async fn get(db: Arc<Db>, ty: GetSubsonicAlbumsListType) -> Vec<String> {
        let results = get_album_list_impl(&db, Default::default(), ty)
            .await
            .unwrap();

        results
            .album_list
            .album
            .iter()
            .map(|t| t.title.clone())
            .collect_vec()
    }
}
