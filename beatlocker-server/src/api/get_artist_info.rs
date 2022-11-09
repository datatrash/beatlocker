use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::XmlStringWrapper;
use crate::{get_lastfm, AppResult, Db, Deserialize, LastFmArtistResponse, Serialize, SharedState};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetArtistInfoParams {
    id: Uuid,
    #[allow(dead_code)]
    count: Option<usize>,
    #[allow(dead_code)]
    include_not_present: Option<bool>,
}

pub async fn get_artist_info(
    format: SubsonicFormat,
    Query(params): Query<GetArtistInfoParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    match get_artist_info_impl(
        &state.read().await.db,
        params,
        state.read().await.options.lastfm_api_key.clone(),
        false,
    )
    .await?
    {
        Some(response) => Ok(format.render(response)),
        None => Ok((StatusCode::NOT_FOUND, ()).into_response()),
    }
}

pub async fn get_artist_info2(
    format: SubsonicFormat,
    Query(params): Query<GetArtistInfoParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    match get_artist_info_impl(
        &state.read().await.db,
        params,
        state.read().await.options.lastfm_api_key.clone(),
        true,
    )
    .await?
    {
        Some(response) => Ok(format.render(ArtistInfo2Response {
            artist_info2: response.artist_info,
        })),
        None => Ok((StatusCode::NOT_FOUND, ()).into_response()),
    }
}

async fn get_artist_info_impl(
    db: &Db,
    params: GetArtistInfoParams,
    lastfm_api_key: Option<String>,
    only_check_artist_id: bool,
) -> AppResult<Option<ArtistInfoResponse>> {
    let mut artist = db.find_artist_by_id(params.id).await?;
    if !only_check_artist_id && artist.is_none() {
        if let Some(song) = db.find_song_by_id(params.id).await? {
            if let Some(artist_id) = song.artist_id {
                artist = db.find_artist_by_id(artist_id).await?;
            }
        }
    }
    if !only_check_artist_id && artist.is_none() {
        for song in db.find_songs_by_album_id(params.id).await? {
            if let Some(artist_id) = song.artist_id {
                artist = db.find_artist_by_id(artist_id).await?;
                break;
            }
        }
    }

    match artist {
        Some(artist) => {
            let mut result = ArtistInfo {
                music_brainz_id: artist.musicbrainz_id.clone(),
                ..Default::default()
            };

            if let Some(api_key) = lastfm_api_key {
                let mut query = vec![
                    ("api_key", api_key.as_str()),
                    ("format", "json"),
                    ("method", "artist.getinfo"),
                    ("artist", &artist.name),
                ];

                if let Some(arid) = &artist.musicbrainz_id {
                    query.push(("mbid", arid));
                }

                let resp: Option<LastFmArtistResponse> = get_lastfm(&query).await?;
                if let Some(resp) = resp {
                    if let Some(artist) = resp.artist {
                        result.last_fm_url = artist.url.clone();
                        result.small_image_url = artist.image("small");
                        result.medium_image_url = artist.image("medium");
                        result.large_image_url = artist.image("large");
                        result.biography = artist.bio.map(|b| b.summary);
                    }
                }
            }

            if let Some(cover_art_id) = artist.cover_art_id {
                let url = Some(format!("/rest/getCoverArt.view?id={cover_art_id}"));
                if result.small_image_url.is_none() {
                    result.small_image_url = url.clone();
                }
                if result.medium_image_url.is_none() {
                    result.medium_image_url = url.clone();
                }
                if result.large_image_url.is_none() {
                    result.large_image_url = url;
                }
            }

            if result.biography.is_none() {
                result.biography = Some(artist.name);
            }

            Ok(Some(ArtistInfoResponse {
                artist_info: result,
            }))
        }
        None => Ok(None),
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfoResponse {
    artist_info: ArtistInfo,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfo2Response {
    artist_info2: ArtistInfo,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    biography: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    music_brainz_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_fm_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    small_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    medium_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    large_image_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlArtistInfoResponse {
    ArtistInfo {
        #[serde(skip_serializing_if = "Option::is_none")]
        biography: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        music_brainz_id: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_fm_url: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        small_image_url: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        medium_image_url: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        large_image_url: Option<XmlStringWrapper>,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlArtistInfo2Response {
    ArtistInfo2 {
        #[serde(skip_serializing_if = "Option::is_none")]
        biography: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        music_brainz_id: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_fm_url: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        small_image_url: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        medium_image_url: Option<XmlStringWrapper>,
        #[serde(skip_serializing_if = "Option::is_none")]
        large_image_url: Option<XmlStringWrapper>,
    },
}

impl ToXml for ArtistInfoResponse {
    type Output = XmlArtistInfoResponse;

    fn into_xml(self) -> Self::Output {
        XmlArtistInfoResponse::ArtistInfo {
            biography: self.artist_info.biography.map(XmlStringWrapper),
            music_brainz_id: self.artist_info.music_brainz_id.map(XmlStringWrapper),
            last_fm_url: self.artist_info.last_fm_url.map(XmlStringWrapper),
            small_image_url: self.artist_info.small_image_url.map(XmlStringWrapper),
            medium_image_url: self.artist_info.medium_image_url.map(XmlStringWrapper),
            large_image_url: self.artist_info.large_image_url.map(XmlStringWrapper),
        }
    }
}

impl ToXml for ArtistInfo2Response {
    type Output = XmlArtistInfo2Response;

    fn into_xml(self) -> Self::Output {
        XmlArtistInfo2Response::ArtistInfo2 {
            biography: self.artist_info2.biography.map(XmlStringWrapper),
            music_brainz_id: self.artist_info2.music_brainz_id.map(XmlStringWrapper),
            last_fm_url: self.artist_info2.last_fm_url.map(XmlStringWrapper),
            small_image_url: self.artist_info2.small_image_url.map(XmlStringWrapper),
            medium_image_url: self.artist_info2.medium_image_url.map(XmlStringWrapper),
            large_image_url: self.artist_info2.large_image_url.map(XmlStringWrapper),
        }
    }
}
