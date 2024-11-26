use crate::http::error::{HttpError, HttpResult};

pub async fn handle_not_found() -> HttpResult<()> {
    Err(HttpError::NotFound.into())
}
