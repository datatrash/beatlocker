use crate::api::format::{SubsonicFormat, ToXml};
use crate::api::model::{SubsonicAlbum, SubsonicArtist, SubsonicSong};
use crate::api::queries::{
    get_subsonic_albums_by_id3, get_subsonic_artists, get_subsonic_songs, GetSubsonicAlbumsQuery,
    GetSubsonicArtistsQuery, GetSubsonicSongsQuery,
};
use crate::{AppResult, SharedState};
use axum::extract::{Query, State};
use axum::response::Response;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::ops::DerefMut;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Search3Params {
    // We don't use 'query', but it's required
    #[allow(dead_code)]
    query: String,
    artist_count: Option<u32>,
    artist_offset: Option<u32>,
    album_count: Option<u32>,
    album_offset: Option<u32>,
    song_count: Option<u32>,
    song_offset: Option<u32>,
    //music_folder_id: Option<Uuid>,
}

pub async fn search3(
    format: SubsonicFormat,
    Query(params): Query<Search3Params>,
    State(state): State<SharedState>,
) -> AppResult<Response> {
    let mut conn = state.db.conn().await?;

    let songs = get_subsonic_songs(
        &mut conn,
        GetSubsonicSongsQuery {
            song_offset: params.song_offset.unwrap_or_default(),
            song_count: params.song_count.unwrap_or(20),
            ..Default::default()
        },
    )
    .await?;

    let artists = get_subsonic_artists(
        conn.deref_mut(),
        GetSubsonicArtistsQuery {
            artist_offset: params.artist_offset.unwrap_or_default(),
            artist_count: params.artist_count.unwrap_or(20),
            ..Default::default()
        },
    )
    .await?;

    let albums = get_subsonic_albums_by_id3(
        conn.deref_mut(),
        GetSubsonicAlbumsQuery {
            offset: params.album_offset.unwrap_or_default(),
            size: params.album_count.unwrap_or(20),
            ..Default::default()
        },
    )
    .await?;

    Ok(format.render(SearchResult3Response {
        search_result3: {
            SearchResult3 {
                album: albums,
                artist: artists,
                song: songs,
            }
        },
    }))
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult3Response {
    search_result3: SearchResult3,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult3 {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    album: Vec<SubsonicAlbum>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    artist: Vec<SubsonicArtist>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    song: Vec<SubsonicSong>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum XmlSearchResult3 {
    #[serde(rename = "searchResult3")]
    SearchResult3(Vec<SubsonicItem>),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SubsonicItem {
    Album(SubsonicAlbum),
    Artist(SubsonicArtist),
    Song(SubsonicSong),
}

impl ToXml for SearchResult3Response {
    type Output = XmlSearchResult3;

    fn into_xml(self) -> Self::Output {
        let mut items: Vec<SubsonicItem> = vec![];
        items.extend(
            self.search_result3
                .album
                .into_iter()
                .map(SubsonicItem::Album)
                .collect_vec(),
        );
        items.extend(
            self.search_result3
                .artist
                .into_iter()
                .map(SubsonicItem::Artist)
                .collect_vec(),
        );
        items.extend(
            self.search_result3
                .song
                .into_iter()
                .map(SubsonicItem::Song)
                .collect_vec(),
        );

        XmlSearchResult3::SearchResult3(items)
    }
}
