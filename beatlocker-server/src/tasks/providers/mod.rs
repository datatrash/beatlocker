pub mod discogs;
pub mod musicbrainz;

use crate::AppResult;
use axum::async_trait;
use chrono::{DateTime, Utc};
pub use discogs::*;
pub use musicbrainz::*;

#[allow(clippy::needless_lifetimes)]
#[async_trait]
pub trait InfoProvider {
    async fn find_release<'a>(&self, query: &FindReleaseQuery<'a>) -> AppResult<Option<Release>>;
    async fn find_cover_art<'a>(&self, query: &FindCoverArtQuery<'a>) -> AppResult<Option<String>>;
    async fn find_artist_photo<'a>(
        &self,
        query: &FindCoverArtQuery<'a>,
    ) -> AppResult<Option<String>>;
}

pub struct InfoProviderOptions {
    pub discogs_token: Option<String>,
}

pub struct InfoProviderList {
    providers: Vec<Box<dyn InfoProvider + Send + Sync>>,
}

impl InfoProviderList {
    pub fn new(options: &InfoProviderOptions) -> Self {
        let mut providers: Vec<Box<dyn InfoProvider + Send + Sync>> =
            vec![Box::new(MbProvider::new())];
        if let Some(token) = &options.discogs_token {
            providers.push(Box::new(DiscogsProvider::new(token)));
        }

        Self { providers }
    }
}

#[async_trait]
impl InfoProvider for InfoProviderList {
    async fn find_release<'a>(&self, query: &FindReleaseQuery<'a>) -> AppResult<Option<Release>> {
        for provider in &self.providers {
            if let Some(result) = provider.find_release(query).await? {
                return Ok(Some(result));
            }
        }

        Ok(None)
    }

    async fn find_cover_art<'a>(&self, query: &FindCoverArtQuery<'a>) -> AppResult<Option<String>> {
        for provider in &self.providers {
            if let Some(result) = provider.find_cover_art(query).await? {
                return Ok(Some(result));
            }
        }

        Ok(None)
    }

    async fn find_artist_photo<'a>(
        &self,
        query: &FindCoverArtQuery<'a>,
    ) -> AppResult<Option<String>> {
        for provider in &self.providers {
            if let Some(result) = provider.find_artist_photo(query).await? {
                return Ok(Some(result));
            }
        }

        Ok(None)
    }
}

pub struct ProviderUri(String);

impl ToString for ProviderUri {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl ProviderUri {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_provider(provider: &str, uri: &str) -> Self {
        Self(format!("{provider}:{uri}"))
    }
}

pub struct FindReleaseQuery<'a> {
    pub album: Option<&'a str>,
    pub artist: &'a str,
    pub song_title: Option<&'a str>,
}

pub struct FindCoverArtQuery<'a> {
    pub album: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub song_title: Option<&'a str>,
}

pub struct Release {
    pub album: Option<(ProviderUri, String)>,
    pub album_artist: Option<(ProviderUri, String)>,
    pub artist: Option<(ProviderUri, String)>,
    pub song: (ProviderUri, String),
    pub genre: Option<String>,
    pub release_date: Option<DateTime<Utc>>,
}
