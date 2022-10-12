use crate::tasks::providers::{FindCoverArtQuery, FindReleaseQuery, InfoProvider, Release};
use crate::{reqwest_client, AppResult};
use axum::async_trait;
use reqwest::header::CONTENT_TYPE;
use reqwest::Method;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct DiscogsSearchResponse {
    results: Vec<DiscogsSearchResult>,
}

#[derive(Debug, Deserialize)]
struct DiscogsSearchResult {
    resource_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscogsResourceResponse {
    artists: Option<Vec<DiscogsArtist>>,
}

#[derive(Debug, Deserialize)]
struct DiscogsArtist {
    thumbnail_url: Option<String>,
}

pub struct DiscogsProvider {
    token: String,
}

impl DiscogsProvider {
    pub fn new(token: &str) -> Self {
        Self {
            token: token.to_string(),
        }
    }
}

#[allow(clippy::needless_lifetimes)]
#[async_trait]
impl InfoProvider for DiscogsProvider {
    async fn find_release<'a>(&self, _query: &FindReleaseQuery<'a>) -> AppResult<Option<Release>> {
        Ok(None)
    }

    async fn find_cover_art<'a>(
        &self,
        _query: &FindCoverArtQuery<'a>,
    ) -> AppResult<Option<String>> {
        Ok(None)
    }

    async fn find_artist_photo<'a>(
        &self,
        query: &FindCoverArtQuery<'a>,
    ) -> AppResult<Option<String>> {
        let response = reqwest_client()
            .request(Method::GET, "https://api.discogs.com/database/search")
            .header(CONTENT_TYPE, "application/json")
            .query(&[
                ("release_title", query.album.unwrap_or_default()),
                ("artist", query.artist.unwrap_or_default()),
                ("token", &self.token),
            ])
            .send()
            .await?;
        let search_response = response.json::<DiscogsSearchResponse>().await?;
        for result in search_response.results {
            if let Some(resource_url) = &result.resource_url {
                let response = reqwest_client()
                    .request(Method::GET, resource_url)
                    .query(&[("token", &self.token)])
                    .send()
                    .await?;
                let resource_response = response.json::<DiscogsResourceResponse>().await?;
                let artists = resource_response.artists.unwrap_or_default();
                for artist in artists {
                    if artist.thumbnail_url.is_some() {
                        return Ok(artist.thumbnail_url);
                    }
                }
            }
        }
        Ok(None)
    }
}
