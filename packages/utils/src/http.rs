pub mod auth {
    use axum::{
        body::Body,
        extract::{Request, State},
        http::{header, Response, StatusCode},
        middleware::Next,
        response::IntoResponse,
    };

    // Shared bearer token middleware with realm support
    // State is a tuple: (token, realm)
    pub async fn verify_bearer_with_realm(
        State((token, realm)): State<(String, String)>,
        req: Request,
        next: Next,
    ) -> impl IntoResponse {
        let unauthorized = || {
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::WWW_AUTHENTICATE, format!("Bearer realm=\"{}\"", realm))
                .body(Body::from("Unauthorized"))
                .unwrap()
        };

        let header_val = match req.headers().get(header::AUTHORIZATION) {
            Some(h) => h,
            None => return unauthorized(),
        };

        let Ok(as_str) = header_val.to_str() else { return unauthorized() };
        let expected = format!("Bearer {}", token);
        if as_str != expected {
            return unauthorized();
        }

        next.run(req).await
    }
}

