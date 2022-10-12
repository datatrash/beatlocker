use crate::api::format::{SubsonicFormat, ToXml};
use crate::AppResult;

use axum::response::Response;
use serde::Serialize;

pub async fn get_license(format: SubsonicFormat) -> AppResult<Response> {
    Ok(format.render(LicenseResponse::License { valid: true }))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LicenseResponse {
    #[serde(rename_all = "camelCase")]
    License { valid: bool },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum XmlLicenseResponse {
    #[serde(rename_all = "camelCase")]
    License { valid: bool },
}

impl ToXml for LicenseResponse {
    type Output = XmlLicenseResponse;

    fn into_xml(self) -> Self::Output {
        match self {
            LicenseResponse::License { valid } => XmlLicenseResponse::License { valid },
        }
    }
}
