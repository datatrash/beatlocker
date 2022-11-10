use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::{SubsonicAlbum, SubsonicArtist, SubsonicSong};
use crate::api::queries::{
    get_subsonic_albums_by_id3, get_subsonic_artists, get_subsonic_songs, GetSubsonicAlbumsQuery,
    GetSubsonicArtistsQuery, GetSubsonicSongsQuery,
};
use crate::{AppResult, Db, Deserialize, Serialize, SharedState};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::ops::DerefMut;
use uuid::Uuid;

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetArtistParams {
    id: Uuid,
}

pub async fn get_artist(
    format: SubsonicFormat,
    Query(params): Query<GetArtistParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    match get_artist_impl(&state.db, params).await? {
        Some(response) => Ok(format.render(response)),
        None => Ok((StatusCode::NOT_FOUND, ()).into_response()),
    }
}

async fn get_artist_impl(db: &Db, params: GetArtistParams) -> AppResult<Option<ArtistResponse>> {
    match get_subsonic_artists(
        db.conn().await?.deref_mut(),
        GetSubsonicArtistsQuery {
            artist_id: Some(params.id),
            ..Default::default()
        },
    )
    .await?
    .first()
    {
        Some(artist) => {
            let albums = get_subsonic_albums_by_id3(
                db.conn().await?.deref_mut(),
                GetSubsonicAlbumsQuery {
                    artist_id: Some(params.id),
                    ..Default::default()
                },
            )
            .await?;

            let songs = get_subsonic_songs(
                db.conn().await?.deref_mut(),
                GetSubsonicSongsQuery {
                    artist_id: Some(params.id),
                    ..Default::default()
                },
            )
            .await?;

            Ok(Some(ArtistResponse {
                artist: SubsonicArtist {
                    album: albums,
                    song: songs,
                    ..artist.clone()
                },
            }))
        }
        None => Ok(None),
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistResponse {
    artist: SubsonicArtist,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlArtistResponse {
    #[serde(rename_all = "camelCase")]
    Artist {
        id: Uuid,
        name: String,
        album_count: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        cover_art: Option<Uuid>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        album: Vec<SubsonicAlbum>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        song: Vec<SubsonicSong>,
    },
}

impl ToXml for ArtistResponse {
    type Output = XmlArtistResponse;

    fn into_xml(self) -> Self::Output {
        XmlArtistResponse::Artist {
            id: self.artist.id,
            name: self.artist.name,
            album_count: self.artist.album_count,
            cover_art: self.artist.cover_art,
            album: self.artist.album,
            song: self.artist.song,
        }
    }
}
