use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::queries::{get_subsonic_artists, GetSubsonicArtistsQuery};
use crate::{AppResult, Db, Deserialize, Serialize, SharedState};
use axum::extract::{Query, State};
use axum::response::Response;
use itertools::Itertools;
use std::ops::DerefMut;
use uuid::Uuid;

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetArtistsParams {
    //music_folder_id: Option<Uuid>,
}

pub async fn get_artists(
    format: SubsonicFormat,
    Query(params): Query<GetArtistsParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    Ok(format.render(get_artists_impl(&state.read().await.db, params).await?))
}

async fn get_artists_impl(db: &Db, _params: GetArtistsParams) -> AppResult<ArtistsResponse> {
    let artists = get_subsonic_artists(
        db.conn().await?.deref_mut(),
        GetSubsonicArtistsQuery::default(),
    )
    .await?;

    let index = artists
        .into_iter()
        .group_by(|ia| ia.name.chars().next().unwrap_or_default())
        .into_iter()
        .map(|(index, artist)| Index {
            name: index.to_string(),
            artist: artist
                .map(|a| IndexArtist {
                    id: a.id,
                    name: a.name,
                    album_count: a.album_count as usize,
                })
                .collect_vec(),
        })
        .sorted_by_key(|index| index.name.clone())
        .collect();

    Ok(ArtistsResponse {
        artists: Indexes { index },
        ignored_articles: "The El La Los Las Le Les Os As O A".to_owned(),
    })
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistsResponse {
    artists: Indexes,
    //last_modified: usize,
    ignored_articles: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Indexes {
    index: Vec<Index>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    name: String,
    artist: Vec<IndexArtist>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexArtist {
    id: Uuid,
    name: String,
    album_count: usize,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlArtistsResponse {
    #[serde(rename_all = "camelCase")]
    Artists {
        index: Vec<Index>,
        ignored_articles: String,
    },
}

impl ToXml for ArtistsResponse {
    type Output = XmlArtistsResponse;

    fn into_xml(self) -> Self::Output {
        XmlArtistsResponse::Artists {
            index: self.artists.index,
            ignored_articles: self.ignored_articles,
        }
    }
}
