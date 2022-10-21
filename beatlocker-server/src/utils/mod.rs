use crate::{AppResult, ExponentialBackoff, HeaderMap, USER_AGENT};
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::RetryTransientMiddleware;
use siphasher::sip128::{Hasher128, SipHasher};
use std::future::Future;
use std::hash::Hash;
use std::time::Duration;
use tracing::warn;
use uuid::Uuid;

mod api_clients;
mod rate_limiter;

pub use api_clients::*;
pub use rate_limiter::RateLimiterMiddleware;

pub fn str_to_uuid(str: &str) -> Uuid {
    let mut h = SipHasher::new();
    str.hash(&mut h);
    let result = h.finish128();
    Uuid::from_u64_pair(result.h1, result.h2)
}

static REQWEST_CLIENT: once_cell::sync::OnceCell<ClientWithMiddleware> =
    once_cell::sync::OnceCell::new();

pub fn reqwest_client() -> &'static ClientWithMiddleware {
    REQWEST_CLIENT.get_or_init(|| {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        reqwest_client_builder()
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    })
}

pub fn reqwest_client_builder() -> reqwest_middleware::ClientBuilder {
    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", USER_AGENT.parse().unwrap());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(5))
        .default_headers(headers)
        .build()
        .unwrap();

    reqwest_middleware::ClientBuilder::new(client)
}

pub async fn wrap_err<T>(
    result: impl Future<Output = AppResult<T>>,
    fallback: impl FnOnce() -> T,
) -> T {
    match result.await {
        Ok(result) => result,
        Err(e) => {
            warn!(?e, "There was an issue during processing");
            fallback()
        }
    }
}
