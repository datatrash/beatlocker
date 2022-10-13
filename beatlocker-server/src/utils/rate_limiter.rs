use axum::async_trait;
use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use governor::{clock, Jitter, Quota, RateLimiter};
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next};
use std::sync::Arc;
use std::time::Duration;
use task_local_extensions::Extensions;

pub struct RateLimiterMiddleware {
    lim: Arc<RateLimiter<NotKeyed, InMemoryState, clock::DefaultClock, NoOpMiddleware>>,
    jitter: Jitter,
}

impl RateLimiterMiddleware {
    pub fn new(quota: Quota) -> Self {
        Self {
            lim: Arc::new(RateLimiter::direct(quota)),
            jitter: Jitter::new(Duration::from_secs(1), Duration::from_secs(1)),
        }
    }
}

#[async_trait]
impl Middleware for RateLimiterMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        self.lim.until_ready_with_jitter(self.jitter).await;
        next.run(req, extensions).await
    }
}
