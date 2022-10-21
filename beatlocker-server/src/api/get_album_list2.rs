use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::queries::{
    get_subsonic_albums_by_id3, GetSubsonicAlbumsListType, GetSubsonicAlbumsQuery,
};
use crate::{AppResult, AppState, Db};
use axum::extract::{Query, State};
use axum::response::Response;

use crate::api::model::SubsonicAlbum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAlbumList2Params {
    //music_folder_id: Option<Uuid>,
    size: Option<u32>,
    offset: Option<u32>,
}

pub async fn get_album_list2(
    format: SubsonicFormat,
    ty: GetSubsonicAlbumsListType,
    Query(params): Query<GetAlbumList2Params>,
    State(state): State<AppState>,
) -> AppResult<Response> {
    Ok(format.render(get_album_list2_impl(&state.db, params, ty).await?))
}

async fn get_album_list2_impl(
    db: &Db,
    params: GetAlbumList2Params,
    ty: GetSubsonicAlbumsListType,
) -> AppResult<AlbumList2Response> {
    let mut conn = db.conn().await?;

    let results = get_subsonic_albums_by_id3(
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

    Ok(AlbumList2Response {
        album_list2: { AlbumList { album: results } },
    })
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumList2Response {
    album_list2: AlbumList,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumList {
    album: Vec<SubsonicAlbum>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlAlbumList2Response {
    #[serde(rename_all = "camelCase")]
    AlbumList2(Vec<SubsonicAlbum>),
}

impl ToXml for AlbumList2Response {
    type Output = XmlAlbumList2Response;

    fn into_xml(self) -> Self::Output {
        XmlAlbumList2Response::AlbumList2(
            self.album_list2
                .album
                .into_iter()
                .map(|a| SubsonicAlbum {
                    id: a.id,
                    name: a.title.clone(),
                    title: a.title,
                    song_count: a.song_count,
                    duration: a.duration,
                    artist: a.artist,
                    artist_id: a.artist_id,
                    cover_art: a.cover_art,
                    ..Default::default()
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
            get(state.db(), GetSubsonicAlbumsListType::AlphabeticalByName).await,
            &["Artist1_Album1", "Artist2_Album1", "SharedAlbum"]
        );
    }

    #[tokio::test]
    async fn sort_alphabetical_by_artist() {
        let state = TestState::new().await.unwrap();

        assert_eq!(
            get(state.db(), GetSubsonicAlbumsListType::AlphabeticalByArtist).await,
            &["Artist1_Album1", "Artist2_Album1", "SharedAlbum"]
        );
    }

    #[tokio::test]
    async fn query_by_year() {
        let state = TestState::new().await.unwrap();

        assert_eq!(
            get(
                state.db(),
                GetSubsonicAlbumsListType::ByYear {
                    from_year: 2014,
                    to_year: 2022
                }
            )
            .await,
            &["SharedAlbum", "Artist2_Album1"]
        );

        assert_eq!(
            get(
                state.db(),
                GetSubsonicAlbumsListType::ByYear {
                    from_year: 2022,
                    to_year: 2014
                }
            )
            .await,
            &["Artist2_Album1", "SharedAlbum"]
        );
    }

    #[tokio::test]
    async fn query_by_genre() {
        let state = TestState::new().await.unwrap();

        assert_eq!(
            get(
                state.db(),
                GetSubsonicAlbumsListType::ByGenre {
                    genre: "Genre1".to_string()
                }
            )
            .await,
            &["Artist1_Album1"]
        );
    }

    async fn get(db: Arc<Db>, ty: GetSubsonicAlbumsListType) -> Vec<String> {
        let results = get_album_list2_impl(&db, Default::default(), ty)
            .await
            .unwrap();

        results
            .album_list2
            .album
            .iter()
            .map(|t| t.title.clone())
            .collect_vec()
    }
}
