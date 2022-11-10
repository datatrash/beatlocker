use crate::SharedState;
use axum::extract::{FromRef, FromRequestParts, Query, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::{async_trait, Json};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

static SUBSONIC_API_VERSION: &str = "1.16.1";

pub struct SubsonicFormat {
    content_type: SubsonicContentType,
    server_version: String,
}

pub enum SubsonicContentType {
    Json,
    Xml,
}

impl SubsonicFormat {
    pub fn render<T>(self, data: impl Into<Option<T>>) -> Response
    where
        T: Clone + Debug + Serialize + ToXml,
    {
        let data = data.into();

        match &self.content_type {
            SubsonicContentType::Json => Json(JsonSubsonicResponse {
                subsonic_response: SubsonicResponse {
                    status: "ok".to_string(),
                    version: SUBSONIC_API_VERSION.to_owned(),
                    ty: "beatlocker".into(),
                    server_version: self.server_version,
                    data,
                },
            })
            .into_response(),
            SubsonicContentType::Xml => {
                let xml = XmlSubsonicResponse {
                    status: "ok".to_owned(),
                    version: SUBSONIC_API_VERSION.to_owned(),
                    ty: "beatlocker".into(),
                    server_version: self.server_version,
                    data: data.map(|d| d.into_xml()),
                };

                let mut bytes = Vec::new();
                quick_xml::se::to_writer(&mut bytes, &xml).unwrap();
                (
                    [(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("application/xml"),
                    )],
                    bytes,
                )
                    .into_response()
            }
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for SubsonicFormat
where
    S: Send + Sync,
    SharedState: FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state: State<SharedState> = State::from_request_parts(parts, state).await?;
        let server_version = app_state.options.server_version.clone();

        #[derive(Deserialize)]
        struct FormatQuery {
            f: String,
        }

        let Query(query) = match Query::<FormatQuery>::from_request_parts(parts, state).await {
            Ok(query) => query,
            Err(_) => {
                return Ok(SubsonicFormat {
                    content_type: SubsonicContentType::Xml,
                    server_version,
                })
            }
        };

        if query.f == "json" {
            Ok(SubsonicFormat {
                content_type: SubsonicContentType::Json,
                server_version,
            })
        } else {
            Ok(SubsonicFormat {
                content_type: SubsonicContentType::Xml,
                server_version,
            })
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JsonSubsonicResponse<T: Clone + Debug + Serialize> {
    #[serde(rename = "subsonic-response")]
    subsonic_response: SubsonicResponse<T>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "subsonic-response", rename_all = "camelCase")]
pub struct SubsonicResponse<T: Clone + Debug + Serialize> {
    status: String,
    version: String,
    #[serde(rename = "type")]
    ty: String,
    server_version: String,
    #[serde(flatten)]
    data: Option<T>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename = "subsonic-response", rename_all = "camelCase")]
pub struct XmlSubsonicResponse<T: Clone + Debug + Serialize> {
    status: String,
    version: String,
    #[serde(rename = "type")]
    ty: String,
    server_version: String,
    #[serde(rename = "$value")]
    data: Option<T>,
}

pub trait ToXml {
    type Output: Clone + Debug + Serialize;
    fn into_xml(self) -> Self::Output;
}

impl ToXml for () {
    type Output = ();

    fn into_xml(self) -> Self::Output {
        self
    }
}

impl<T: Clone + Debug + Serialize + ToXml> ToXml for SubsonicResponse<T> {
    type Output = XmlSubsonicResponse<T>;

    fn into_xml(self: SubsonicResponse<T>) -> Self::Output {
        XmlSubsonicResponse {
            status: self.status,
            version: self.version,
            ty: self.ty,
            server_version: self.server_version,
            data: self.data,
        }
    }
}
