use crate::tasks::providers::{FindCoverArtQuery, FindReleaseQuery, InfoProvider, ProviderUri};
use crate::AppResult;
use axum::async_trait;
use chrono::{NaiveTime, Utc};
use distance::damerau_levenshtein;
use itertools::Itertools;
use musicbrainz_rs::entity::recording::{Recording, RecordingSearchQuery};
use musicbrainz_rs::entity::release::Release;
use musicbrainz_rs::entity::CoverartResponse;
use musicbrainz_rs::{FetchCoverart, Search};
use tracing::{info, warn};

const PROVIDER_ID: &str = "mb";

pub struct MbProvider {}

impl MbProvider {
    pub fn new() -> Self {
        Self {}
    }
}

#[allow(clippy::needless_lifetimes)]
#[async_trait]
impl InfoProvider for MbProvider {
    async fn find_release<'a>(
        &self,
        query: &FindReleaseQuery<'a>,
    ) -> AppResult<Option<super::Release>> {
        let r = find_recording_releases(query.album, Some(query.artist), query.song_title).await?;
        Ok(r.first().map(|(recording, release)| {
            let artist = recording
                .artist_credit
                .clone()
                .unwrap_or_default()
                .first()
                .map(|credit| {
                    (
                        ProviderUri::from_provider(PROVIDER_ID, &credit.artist.id),
                        credit.name.clone(),
                    )
                });

            let album_artist = release
                .artist_credit
                .clone()
                .unwrap_or_default()
                .first()
                .map(|credit| {
                    (
                        ProviderUri::from_provider(PROVIDER_ID, &credit.artist.id),
                        credit.name.clone(),
                    )
                });

            let genre = recording
                .genres
                .clone()
                .unwrap_or_default()
                .first()
                .map(|g| g.name.clone());

            super::Release {
                album: Some((
                    ProviderUri::from_provider(PROVIDER_ID, &release.id),
                    release.title.clone(),
                )),
                album_artist,
                artist,
                song: (
                    ProviderUri::from_provider(PROVIDER_ID, &recording.id),
                    recording.title.clone(),
                ),
                genre,
                release_date: release.date.map(|date| {
                    date.and_time(NaiveTime::default())
                        .and_local_timezone(Utc)
                        .unwrap()
                }),
            }
        }))
    }

    async fn find_cover_art<'a>(&self, query: &FindCoverArtQuery<'a>) -> AppResult<Option<String>> {
        let r = find_recording_releases(query.album, query.artist, query.song_title).await?;
        for (_, release) in r {
            if let Ok(CoverartResponse::Url(coverart_url)) = Release::fetch_coverart()
                .id(&release.id)
                .res_500()
                .front()
                .execute()
                .await
                .map_err(|e| {
                    info!(?e, "Could not fetch cover art");
                })
            {
                if !coverart_url.starts_with("http://coverartarchive.org") {
                    return Ok(Some(coverart_url));
                }
            }
        }

        Ok(None)
    }

    async fn find_artist_photo<'a>(
        &self,
        _query: &FindCoverArtQuery<'a>,
    ) -> AppResult<Option<String>> {
        Ok(None)
    }
}

async fn find_recording_releases(
    album: Option<&str>,
    artist: Option<&str>,
    song_title: Option<&str>,
) -> AppResult<Vec<(Recording, Release)>> {
    let mut query_builder = RecordingSearchQuery::query_builder();
    if let Some(artist) = artist {
        query_builder.and().artist_name(artist);
    }

    if let Some(title) = &song_title {
        query_builder.and().recording(title);
    }

    if let Some(album) = &album {
        query_builder.and().release(album);
    }

    match Recording::search(query_builder.build()).execute().await {
        Ok(recordings) => Ok(recordings
            .entities
            .into_iter()
            .flat_map(|recording| {
                let releases = recording.releases.clone().unwrap_or_default();
                releases
                    .into_iter()
                    .map(|release| (recording.clone(), release))
                    .collect_vec()
            })
            .sorted_by_key(|(_rec, rel)| {
                let max_track_count = if let Some(media) = &rel.media {
                    media
                        .iter()
                        .map(|m| m.track_count)
                        .max()
                        .unwrap_or_default()
                } else {
                    0
                };
                let album = album.unwrap_or_default();
                let distance = damerau_levenshtein(album, &rel.title);
                (max_track_count, distance)
            })
            .collect_vec()),
        Err(e) => {
            warn!(?e, "Could not retrieve releases");
            Ok(vec![])
        }
    }
}
