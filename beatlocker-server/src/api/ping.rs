use crate::api::format::SubsonicFormat;

use axum::response::Response;

pub async fn ping(format: SubsonicFormat) -> Response {
    format.render::<()>(None)
}
