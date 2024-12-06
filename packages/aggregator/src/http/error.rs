// Work-in-progress, but big picture ideas:
// handlers return `HttpResult`
// this allows us to use `?` in handlers with anything anyhow-friendly
// specific errors can formed by returning `HttpError` can be recovered via downcast
// and HttpError knows how to become a specific HttpResponse with status code
use axum::body::Body;
use axum::http::{Response, StatusCode};
use axum::response::IntoResponse;

pub type HttpResult<T> = std::result::Result<T, AnyError>;

#[derive(thiserror::Error, Debug)]
pub enum HttpError {
    #[error("not found")]
    NotFound,
}

// Make our own error that wraps `anyhow::Error`.
#[derive(Debug)]
pub struct AnyError(anyhow::Error);

impl IntoResponse for AnyError {
    fn into_response(self) -> Response<Body> {
        match self.0.downcast::<HttpError>() {
            Ok(app_error) => app_error.into_response(),
            Err(e) => {
                let e = e.to_string();
                tracing::error!("{}", e);

                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(e.into())
                    .unwrap()
            }
        }
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response<Body> {
        let status = match &self {
            HttpError::NotFound => StatusCode::NOT_FOUND,
        };

        let body = self.to_string().into();

        Response::builder()
            .status(status)
            .body(body)
            .expect("failed to render response")
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AnyError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
