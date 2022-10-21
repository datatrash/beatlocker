use crate::{reqwest_client_builder, AppResult, RateLimiterMiddleware};
use governor::Quota;
use http_cache_reqwest::{Cache, CacheMode, HttpCache, MokaManager};
use reqwest::header::CONTENT_TYPE;
use reqwest::{Method, StatusCode};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::num::NonZeroU32;
use std::time::Duration;
use tracing::{debug, error};

#[derive(Debug, Deserialize)]
pub struct DiscogsSearchResponse {
    pub results: Vec<DiscogsSearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct DiscogsSearchResult {
    pub genre: Vec<String>,
    pub cover_image: Option<String>,
    pub thumb: Option<String>,
    pub master_url: Option<String>,
    pub resource_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiscogsMasterResponse {
    #[serde(default)]
    pub images: Vec<DiscogsImage>,
}

#[derive(Debug, Deserialize)]
pub struct DiscogsResourceResponse {
    #[serde(default)]
    pub artists: Vec<DiscogsArtist>,
}

#[derive(Debug, Deserialize)]
pub struct DiscogsArtist {
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiscogsImage {
    pub resource_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MusicbrainzRecordingsResponse {
    pub recordings: Vec<MusicbrainzRecording>,
}

#[derive(Debug, Deserialize)]
pub struct MusicbrainzRecording {
    #[serde(default, rename = "artist-credit")]
    pub artist_credit: Vec<MusicbrainzArtistCredit>,
    #[serde(default)]
    pub releases: Vec<MusicbrainzRelease>,
    #[serde(default)]
    pub tags: Vec<MusicbrainzTag>,
}

#[derive(Debug, Deserialize)]
pub struct MusicbrainzRelease {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct MusicbrainzArtistCredit {
    pub artist: MusicbrainzArtist,
}

#[derive(Debug, Deserialize)]
pub struct MusicbrainzArtist {
    pub id: String,
    #[serde(default)]
    pub tags: Vec<MusicbrainzTag>,
}

#[derive(Debug, Deserialize)]
pub struct MusicbrainzTag {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct MusicbrainzArtistsResponse {
    pub artists: Vec<MusicbrainzArtist>,
}

#[derive(Debug, Deserialize)]
pub struct CoverArtArchiveImagesResponse {
    #[serde(default)]
    pub images: Vec<CoverArtArchiveImage>,
}

#[derive(Debug, Deserialize)]
pub struct CoverArtArchiveImage {
    pub image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LastFmArtistResponse {
    pub artist: Option<LastFmArtist>,
}

#[derive(Debug, Deserialize)]
pub struct LastFmArtist {
    pub url: Option<String>,
    pub image: Vec<LastFmImage>,
    pub bio: Option<LastFmBio>,
}

impl LastFmArtist {
    pub fn image(&self, size: &str) -> Option<String> {
        self.image
            .iter()
            .find(|i| i.size == size)
            .map(|i| i.text.clone())
    }
}

#[derive(Debug, Deserialize)]
pub struct LastFmImage {
    #[serde(rename = "#text")]
    pub text: String,
    pub size: String,
}

#[derive(Debug, Deserialize)]
pub struct LastFmBio {
    pub summary: String,
}

static DISCOGS_CLIENT: once_cell::sync::OnceCell<ClientWithMiddleware> =
    once_cell::sync::OnceCell::new();

pub fn discogs_client() -> &'static ClientWithMiddleware {
    DISCOGS_CLIENT.get_or_init(|| {
        // Allow 1 request every 2 seconds, otherwise we'll get rate limited
        let quota = Quota::with_period(Duration::from_secs(2)).unwrap();

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(20), Duration::from_secs(300))
            .build_with_max_retries(3);
        reqwest_client_builder()
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .with(default_cache_middleware())
            .with(RateLimiterMiddleware::new(quota))
            .build()
    })
}

static MUSICBRAINZ_CLIENT: once_cell::sync::OnceCell<ClientWithMiddleware> =
    once_cell::sync::OnceCell::new();

fn musicbrainz_client() -> &'static ClientWithMiddleware {
    MUSICBRAINZ_CLIENT.get_or_init(|| {
        let quota = Quota::per_second(NonZeroU32::new(10).unwrap());

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(20), Duration::from_secs(300))
            .build_with_max_retries(3);
        reqwest_client_builder()
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .with(default_cache_middleware())
            .with(RateLimiterMiddleware::new(quota))
            .build()
    })
}

static COVER_ART_ARCHIVE_CLIENT: once_cell::sync::OnceCell<ClientWithMiddleware> =
    once_cell::sync::OnceCell::new();

fn cover_art_archive_client() -> &'static ClientWithMiddleware {
    COVER_ART_ARCHIVE_CLIENT.get_or_init(|| {
        let quota = Quota::per_second(NonZeroU32::new(10).unwrap());

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(20), Duration::from_secs(300))
            .build_with_max_retries(3);
        reqwest_client_builder()
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .with(default_cache_middleware())
            .with(RateLimiterMiddleware::new(quota))
            .build()
    })
}

static LASTFM_CLIENT: once_cell::sync::OnceCell<ClientWithMiddleware> =
    once_cell::sync::OnceCell::new();

fn lastfm_client() -> &'static ClientWithMiddleware {
    LASTFM_CLIENT.get_or_init(|| {
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(20), Duration::from_secs(300))
            .build_with_max_retries(3);
        reqwest_client_builder()
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .with(default_cache_middleware())
            .build()
    })
}

pub async fn get_discogs<T: for<'a> Deserialize<'a>, Q: Serialize + Debug + ?Sized>(
    endpoint: &str,
    query: &Q,
) -> AppResult<Option<T>> {
    debug!(?endpoint, ?query, "Sending discogs query");

    let response = discogs_client()
        .request(
            Method::GET,
            format!("https://api.discogs.com/database/{}", endpoint),
        )
        .header(CONTENT_TYPE, "application/json")
        .query(query)
        .send()
        .await?;

    let status_code = response.status();
    if status_code == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let json = response.text().await?;
    match serde_json::from_str::<T>(&json) {
        Ok(response) => Ok(Some(response)),
        Err(e) => {
            error!(
                ?status_code,
                ?json,
                "Problem decoding Discogs JSON response"
            );
            debug!(?e);
            Ok(None)
        }
    }
}

pub async fn get_musicbrainz<T: for<'a> Deserialize<'a>, Q: Serialize + Debug + ?Sized>(
    endpoint: &str,
    query: &Q,
) -> AppResult<Option<T>> {
    debug!(?endpoint, ?query, "Sending musicbrainz query");

    let response = musicbrainz_client()
        .request(
            Method::GET,
            format!("https://musicbrainz.org/ws/2/{}", endpoint),
        )
        .header(CONTENT_TYPE, "application/json")
        .query(query)
        .send()
        .await?;

    let status_code = response.status();
    if status_code == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let json = response.text().await?;
    match serde_json::from_str::<T>(&json) {
        Ok(response) => Ok(Some(response)),
        Err(e) => {
            error!(
                ?status_code,
                ?json,
                "Problem decoding Musicbrainz JSON response"
            );
            debug!(?e);
            Ok(None)
        }
    }
}

pub async fn get_cover_art_archive<T: for<'a> Deserialize<'a>>(
    endpoint: &str,
    id: &str,
) -> AppResult<Option<T>> {
    debug!(?endpoint, ?id, "Sending cover art archive query");

    let response = cover_art_archive_client()
        .request(
            Method::GET,
            format!("https://coverartarchive.org/{}/{}", endpoint, id),
        )
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await?;

    let status_code = response.status();
    if status_code == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let json = response.text().await?;
    match serde_json::from_str::<T>(&json) {
        Ok(response) => Ok(Some(response)),
        Err(e) => {
            error!(
                ?status_code,
                ?json,
                "Problem decoding Cover Art Archive JSON response"
            );
            debug!(?e);
            Ok(None)
        }
    }
}

pub async fn get_lastfm<T: for<'a> Deserialize<'a>, Q: Serialize + Debug + ?Sized>(
    query: &Q,
) -> AppResult<Option<T>> {
    debug!(?query, "Sending last.fm query");

    let response = lastfm_client()
        .request(Method::GET, "http://ws.audioscrobbler.com/2.0/")
        .header(CONTENT_TYPE, "application/json")
        .query(query)
        .send()
        .await?;

    let status_code = response.status();
    if status_code == StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let json = response.text().await?;
    match serde_json::from_str::<T>(&json) {
        Ok(response) => Ok(Some(response)),
        Err(e) => {
            error!(
                ?status_code,
                ?json,
                "Problem decoding last.fm JSON response"
            );
            debug!(?e);
            Ok(None)
        }
    }
}

fn default_cache_middleware() -> Cache<MokaManager> {
    Cache(HttpCache {
        mode: CacheMode::ForceCache,
        manager: MokaManager::default(),
        options: None,
    })
}
