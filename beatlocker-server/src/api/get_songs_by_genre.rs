use std::ops::DerefMut;

use axum::extract::{Query, State};
use axum::response::Response;

use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::SubsonicSong;
use crate::api::queries::{get_subsonic_songs, GetSubsonicSongsQuery};
use crate::{AppResult, Db, Deserialize, Serialize, SharedState};

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSongsByGenreParams {
    genre: String,
    count: Option<u32>,
    offset: Option<u32>,
    //music_folder_id: Option<Uuid>,
}

pub async fn get_songs_by_genre(
    format: SubsonicFormat,
    Query(params): Query<GetSongsByGenreParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    Ok(format.render(get_songs_by_genre_impl(&state.db, params).await?))
}

async fn get_songs_by_genre_impl(
    db: &Db,
    params: GetSongsByGenreParams,
) -> AppResult<SongsByGenreResponse> {
    let songs = get_subsonic_songs(
        db.conn().await?.deref_mut(),
        GetSubsonicSongsQuery {
            //folder_id: params.music_folder_id,
            album_id: None,
            genre: Some(params.genre),
            song_offset: params.offset.unwrap_or_default(),
            song_count: params.count.unwrap_or(10),
            ..Default::default()
        },
    )
    .await?;

    Ok(SongsByGenreResponse {
        songs_by_genre: SongsByGenre { song: songs },
    })
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongsByGenreResponse {
    songs_by_genre: SongsByGenre,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongsByGenre {
    song: Vec<SubsonicSong>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlSongsByGenreResponse {
    SongsByGenre { song: Vec<SubsonicSong> },
}

impl ToXml for SongsByGenreResponse {
    type Output = XmlSongsByGenreResponse;

    fn into_xml(self) -> Self::Output {
        XmlSongsByGenreResponse::SongsByGenre {
            song: self.songs_by_genre.song,
        }
    }
}
