use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::{SubsonicAlbum, SubsonicSong};
use crate::api::queries::{
    get_subsonic_albums_by_id3, get_subsonic_songs, GetSubsonicAlbumsQuery, GetSubsonicSongsQuery,
};
use crate::{AppResult, Db, Deserialize, Serialize, SharedState};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::ops::DerefMut;

use uuid::Uuid;

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAlbumParams {
    id: Uuid,
}

pub async fn get_album(
    format: SubsonicFormat,
    Query(params): Query<GetAlbumParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    match get_album_impl(&state.read().await.db, params).await? {
        Some(response) => Ok(format.render(response)),
        None => Ok((StatusCode::NOT_FOUND, ()).into_response()),
    }
}

async fn get_album_impl(db: &Db, params: GetAlbumParams) -> AppResult<Option<AlbumResponse>> {
    match get_subsonic_albums_by_id3(
        db.conn().await?.deref_mut(),
        GetSubsonicAlbumsQuery {
            album_id: Some(params.id),
            ..Default::default()
        },
    )
    .await?
    .first()
    {
        Some(album) => {
            let songs = get_subsonic_songs(
                db.conn().await?.deref_mut(),
                GetSubsonicSongsQuery {
                    album_id: Some(params.id),
                    ..Default::default()
                },
            )
            .await?;

            Ok(Some(AlbumResponse {
                album: SubsonicAlbum {
                    song: songs,
                    ..album.clone()
                },
            }))
        }
        None => Ok(None),
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumResponse {
    album: SubsonicAlbum,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlAlbumResponse {
    #[serde(rename_all = "camelCase")]
    Album {
        id: Uuid,
        name: String,
        title: String,
        song_count: u32,
        duration: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        artist: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        artist_id: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cover_art: Option<Uuid>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        song: Vec<SubsonicSong>,
    },
}

impl ToXml for AlbumResponse {
    type Output = XmlAlbumResponse;

    fn into_xml(self) -> Self::Output {
        XmlAlbumResponse::Album {
            id: self.album.id,
            name: self.album.name,
            title: self.album.title,
            song_count: self.album.song_count,
            duration: self.album.duration,
            artist: self.album.artist,
            artist_id: self.album.artist_id,
            cover_art: self.album.cover_art,
            song: self.album.song,
        }
    }
}
