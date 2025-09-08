use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    typed_header::TypedHeaderRejection,
    TypedHeader,
};
use subtle::ConstantTimeEq;

// Shared bearer token middleware with realm support
// State is a tuple: (token, realm)
pub async fn verify_bearer_with_realm(
    State((token, realm)): State<(String, String)>,
    auth: Result<TypedHeader<Authorization<Bearer>>, TypedHeaderRejection>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let unauthorized = |error: &str, description: &str| {
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(
                header::WWW_AUTHENTICATE,
                format!(
                    "Bearer realm=\"{}\", error=\"{}\", error_description=\"{}\"",
                    realm, error, description
                ),
            )
            .body(Body::from("Unauthorized"))
            .unwrap()
    };

    match auth {
        Ok(TypedHeader(Authorization(bearer))) => {
            if bearer.token().as_bytes().ct_eq(token.as_bytes()).into() {
                next.run(req).await
            } else {
                unauthorized("invalid_token", "token_mismatch")
            }
        }
        Err(_) => unauthorized("invalid_request", "invalid_authorization_header"),
    }
}

#[cfg(test)]
mod tests {
    use super::verify_bearer_with_realm;
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::util::ServiceExt; // for `oneshot`

    fn app_with_auth(token: &str, realm: &str) -> Router {
        let protected = Router::new().route("/protected", get(|| async { "ok" }));
        protected.layer(middleware::from_fn_with_state(
            (token.to_string(), realm.to_string()),
            verify_bearer_with_realm,
        ))
    }

    #[tokio::test]
    async fn bearer_auth_missing_header_is_unauthorized() {
        let app = app_with_auth("s3cr3t", "testrealm");

        let req = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let hdr = resp.headers().get(header::WWW_AUTHENTICATE).unwrap();
        let val = hdr.to_str().unwrap();
        assert!(val.contains("Bearer realm=\"testrealm\""));
        assert!(val.contains("error=\"invalid_request\""));
        assert!(val.contains("error_description=\"invalid_authorization_header\""));
    }

    #[tokio::test]
    async fn bearer_auth_wrong_token_is_unauthorized() {
        let app = app_with_auth("s3cr3t", "testrealm");

        let req = Request::builder()
            .uri("/protected")
            .header(header::AUTHORIZATION, "Bearer wrong")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let hdr = resp.headers().get(header::WWW_AUTHENTICATE).unwrap();
        let val = hdr.to_str().unwrap();
        assert!(val.contains("Bearer realm=\"testrealm\""));
        assert!(val.contains("error=\"invalid_token\""));
        assert!(val.contains("error_description=\"token_mismatch\""));
    }

    #[tokio::test]
    async fn bearer_auth_correct_token_allows() {
        let app = app_with_auth("s3cr3t", "testrealm");

        let req = Request::builder()
            .uri("/protected")
            .header(header::AUTHORIZATION, "Bearer s3cr3t")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().get(header::WWW_AUTHENTICATE).is_none());
    }
}
