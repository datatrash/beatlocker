use crate::SubsonicAuth;
use crate::SubsonicAuth::UsernamePassword;
use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use axum::{async_trait, RequestPartsExt, TypedHeader};
use headers::authorization::Basic;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

pub struct RequireAuth;

#[derive(Debug, Deserialize)]
struct AuthQuery {
    u: Option<String>,
    p: Option<String>,
    t: Option<String>,
    s: Option<String>,
}

static AUTH_MUTEX: once_cell::sync::OnceCell<Mutex<()>> = once_cell::sync::OnceCell::new();

pub fn auth_mutex() -> &'static Mutex<()> {
    AUTH_MUTEX.get_or_init(|| Mutex::new(()))
}

#[async_trait]
impl FromRequestParts<SubsonicAuth> for RequireAuth {
    type Rejection = axum::http::StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SubsonicAuth,
    ) -> Result<Self, Self::Rejection> {
        match &state {
            UsernamePassword { username, password } => {
                // Don't allow concurrent login attempts
                let _ = auth_mutex().lock().await;

                let auth_query = if let Ok(header) = parts
                    .extract::<TypedHeader<headers::Authorization<Basic>>>()
                    .await
                {
                    Some(AuthQuery {
                        u: Some(header.username().to_owned()),
                        p: Some(header.password().to_owned()),
                        t: None,
                        s: None,
                    })
                } else {
                    Query::<AuthQuery>::from_request_parts(parts, state)
                        .await
                        .ok()
                        .map(|q| q.0)
                };

                let is_valid = {
                    match auth_query {
                        Some(query) => match (&query.u, &query.p, &query.t, &query.s) {
                            (Some(u), _, Some(t), Some(s)) => {
                                check_user(username, password, u, t, s)
                            }
                            (Some(u), Some(p), _, _) => check_legacy_user(username, password, u, p),
                            _ => false,
                        },
                        None => false,
                    }
                };

                if is_valid {
                    Ok(Self)
                } else {
                    // Wait a bit, to prevent login attempts being spammed
                    sleep(Duration::from_millis(800)).await;
                    Err(axum::http::StatusCode::UNAUTHORIZED)
                }
            }
            SubsonicAuth::None => Ok(Self),
        }
    }
}

fn check_user(username: &str, password: &str, u: &str, t: &str, s: &str) -> bool {
    let digest = md5::compute(format!("{password}{s}"));
    let expected_token = format!("{:02X?}", digest);
    username == u && t.to_lowercase() == expected_token
}

fn check_legacy_user(username: &str, password: &str, u: &str, p: &str) -> bool {
    let p = if p.starts_with("enc:") {
        if let Some((_, encoded)) = p.split_once(':') {
            String::from_utf8_lossy(&hex::decode(encoded).unwrap_or_default()).to_string()
        } else {
            p.to_string()
        }
    } else {
        p.to_string()
    };
    username == u && password == p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_user() {
        assert!(check_user(
            "joe",
            "sesame",
            "joe",
            "26719a1196d2a940705a59634eb18eab",
            "c19b2d"
        ));
        assert!(!check_user(
            "joe",
            "sesame",
            "not-joe",
            "26719a1196d2a940705a59634eb18eab",
            "c19b2d"
        ));
        assert!(!check_user(
            "joe",
            "snuh",
            "joe",
            "26719a1196d2a940705a59634eb18eab",
            "c19b2d"
        ));
    }

    #[test]
    fn test_check_legacy_user() {
        assert!(check_legacy_user("joe", "foo", "joe", "foo"));
        assert!(check_legacy_user("joe", "foo", "joe", "enc:666f6f"));
        assert!(!check_legacy_user("joe", "foo", "joe", "bar"));
        assert!(!check_legacy_user("joe", "foo", "joe", "enc"));
        assert!(!check_legacy_user("joe", "foo", "joe", "enc:"));
        assert!(!check_legacy_user("joe", "foo", "joe", "enc:randomstuff"));
    }
}
