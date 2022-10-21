use std::ops::DerefMut;

use axum::extract::State;
use axum::response::Response;
use itertools::Itertools;

use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::{SubsonicAlbum, SubsonicArtist, SubsonicSong};
use crate::api::queries::{
    get_subsonic_albums_by_id3, get_subsonic_artists, get_subsonic_songs, GetSubsonicAlbumsQuery,
    GetSubsonicArtistsQuery, GetSubsonicSongsQuery,
};
use crate::{AppResult, AppState, Deserialize, Serialize};

pub async fn get_starred2(
    format: SubsonicFormat,
    State(state): State<AppState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

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

    let albums = get_subsonic_albums_by_id3(
        conn.deref_mut(),
        GetSubsonicAlbumsQuery {
            starred: true,
            size: 1000,
            ..Default::default()
        },
    )
    .await?;

    Ok(format.render(Starred2Response {
        starred2: {
            Starred2 {
                album: albums,
                artist: artists,
                song: songs,
            }
        },
    }))
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Starred2Response {
    starred2: Starred2,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Starred2 {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    album: Vec<SubsonicAlbum>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    artist: Vec<SubsonicArtist>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    song: Vec<SubsonicSong>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum XmlStarred2Response {
    #[serde(rename = "starred2")]
    Starred2(Vec<SubsonicItem>),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SubsonicItem {
    Album(SubsonicAlbum),
    Artist(SubsonicArtist),
    Song(SubsonicSong),
}

impl ToXml for Starred2Response {
    type Output = XmlStarred2Response;

    fn into_xml(self) -> Self::Output {
        let mut items: Vec<SubsonicItem> = vec![];
        items.extend(
            self.starred2
                .album
                .into_iter()
                .map(SubsonicItem::Album)
                .collect_vec(),
        );
        items.extend(
            self.starred2
                .artist
                .into_iter()
                .map(SubsonicItem::Artist)
                .collect_vec(),
        );
        items.extend(
            self.starred2
                .song
                .into_iter()
                .map(SubsonicItem::Song)
                .collect_vec(),
        );

        XmlStarred2Response::Starred2(items)
    }
}
