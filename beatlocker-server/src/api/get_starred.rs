use std::ops::DerefMut;

use axum::extract::State;
use axum::response::Response;
use itertools::Itertools;

use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::{SubsonicAlbum, SubsonicArtist, SubsonicSong};
use crate::api::queries::{
    get_subsonic_albums, get_subsonic_artists, get_subsonic_songs, GetSubsonicAlbumsQuery,
    GetSubsonicArtistsQuery, GetSubsonicSongsQuery,
};
use crate::{AppResult, Deserialize, Serialize, SharedState};

pub async fn get_starred(
    format: SubsonicFormat,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    let mut conn = state.read().await.db.conn().await?;

    let songs = get_subsonic_songs(
        &mut conn,
        GetSubsonicSongsQuery {
            starred: true,
            song_count: 1000,
            ..Default::default()
        },
    )
    .await?;

    let artists = get_subsonic_artists(
        conn.deref_mut(),
        GetSubsonicArtistsQuery {
            starred: true,
            artist_count: 1000,
            ..Default::default()
        },
    )
    .await?;

    let albums = get_subsonic_albums(
        conn.deref_mut(),
        GetSubsonicAlbumsQuery {
            starred: true,
            size: 1000,
            ..Default::default()
        },
    )
    .await?;

    Ok(format.render(StarredResponse {
        starred: {
            Starred {
                album: albums,
                artist: artists,
                song: songs,
            }
        },
    }))
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StarredResponse {
    starred: Starred,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Starred {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    album: Vec<SubsonicAlbum>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    artist: Vec<SubsonicArtist>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    song: Vec<SubsonicSong>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum XmlStarredResponse {
    #[serde(rename = "starred")]
    Starred(Vec<SubsonicItem>),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SubsonicItem {
    Album(SubsonicAlbum),
    Artist(SubsonicArtist),
    Song(SubsonicSong),
}

impl ToXml for StarredResponse {
    type Output = XmlStarredResponse;

    fn into_xml(self) -> Self::Output {
        let mut items: Vec<SubsonicItem> = vec![];
        items.extend(
            self.starred
                .album
                .into_iter()
                .map(SubsonicItem::Album)
                .collect_vec(),
        );
        items.extend(
            self.starred
                .artist
                .into_iter()
                .map(SubsonicItem::Artist)
                .collect_vec(),
        );
        items.extend(
            self.starred
                .song
                .into_iter()
                .map(SubsonicItem::Song)
                .collect_vec(),
        );

        XmlStarredResponse::Starred(items)
    }
}
