use std::ops::DerefMut;

use axum::extract::{Query, State};
use axum::response::Response;

use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::SubsonicSong;
use crate::api::queries::{get_subsonic_songs, GetSubsonicSongsQuery};
use crate::{AppResult, Db, Deserialize, Serialize, SharedState};

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRandomSongsParams {
    genre: Option<String>,
    size: Option<u32>,
    from_year: Option<u32>,
    to_year: Option<u32>,
    //music_folder_id: Option<Uuid>,
}

pub async fn get_random_songs(
    format: SubsonicFormat,
    Query(params): Query<GetRandomSongsParams>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    Ok(format.render(get_random_songs_impl(&state.read().await.db, params).await?))
}

async fn get_random_songs_impl(
    db: &Db,
    params: GetRandomSongsParams,
) -> AppResult<RandomSongsResponse> {
    let songs = get_subsonic_songs(
        db.conn().await?.deref_mut(),
        GetSubsonicSongsQuery {
            genre: params.genre,
            song_count: params.size.unwrap_or(10),
            from_year: params.from_year,
            to_year: params.to_year,
            random: true,
            ..Default::default()
        },
    )
    .await?;

    Ok(RandomSongsResponse {
        random_songs: RandomSongs { song: songs },
    })
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomSongsResponse {
    random_songs: RandomSongs,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomSongs {
    song: Vec<SubsonicSong>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum XmlRandomSongsResponse {
    RandomSongs { song: Vec<SubsonicSong> },
}

impl ToXml for RandomSongsResponse {
    type Output = XmlRandomSongsResponse;

    fn into_xml(self) -> Self::Output {
        XmlRandomSongsResponse::RandomSongs {
            song: self.random_songs.song,
        }
    }
}
