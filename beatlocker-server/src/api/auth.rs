use crate::SubsonicAuth;
use crate::SubsonicAuth::UsernamePassword;
use axum::async_trait;
use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use serde::Deserialize;

pub struct RequireAuth;

#[derive(Debug, Deserialize)]
struct AuthQuery {
    u: Option<String>,
    p: Option<String>,
    t: Option<String>,
    s: Option<String>,
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
                let is_valid = {
                    match Query::<AuthQuery>::from_request_parts(parts, state)
                        .await
                        .ok()
                    {
                        Some(query) => match (&query.u, &query.p, &query.t, &query.s) {
                            (_, _, Some(t), Some(s)) => check_user(username, password, &t, &s),
                            (Some(u), Some(p), _, _) => {
                                check_legacy_user(username, password, &u, &p)
                            }
                            _ => false,
                        },
                        None => false,
                    }
                };

                if is_valid {
                    Ok(Self)
                } else {
                    Err(axum::http::StatusCode::UNAUTHORIZED)
                }
            }
            SubsonicAuth::None => Ok(Self),
        }
    }
}

fn check_user(_username: &str, password: &str, t: &str, s: &str) -> bool {
    let digest = md5::compute(format!("{password}{s}"));
    let expected_token = format!("{:02X?}", digest);
    t.to_lowercase() == expected_token
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
        assert_eq!(
            check_user(
                "joe",
                "sesame",
                "26719a1196d2a940705a59634eb18eab",
                "c19b2d"
            ),
            true
        );
        assert_eq!(
            check_user("joe", "snuh", "26719a1196d2a940705a59634eb18eab", "c19b2d"),
            false
        );
    }

    #[test]
    fn test_check_legacy_user() {
        assert_eq!(check_legacy_user("joe", "foo", "joe", "foo"), true);
        assert_eq!(check_legacy_user("joe", "foo", "joe", "enc:666f6f"), true);
        assert_eq!(check_legacy_user("joe", "foo", "joe", "bar"), false);
        assert_eq!(check_legacy_user("joe", "foo", "joe", "enc"), false);
        assert_eq!(check_legacy_user("joe", "foo", "joe", "enc:"), false);
        assert_eq!(
            check_legacy_user("joe", "foo", "joe", "enc:randomstuff"),
            false
        );
    }
}
