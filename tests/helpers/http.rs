use std::sync::Arc;

use wasmatic::dispatcher::mock::MockDispatcher;

use super::app::TestApp;

#[derive(Clone)]
pub struct TestHttpApp {
    pub inner: TestApp,
    _http_router: axum::Router,
}

impl TestHttpApp {
    pub async fn new() -> Self {
        let inner = TestApp::new().await;

        let dispatcher = Arc::new(MockDispatcher::new(inner.config.as_ref().clone()));

        let http_router = wasmatic::http::server::make_router(dispatcher)
            .await
            .unwrap();

        Self {
            inner,
            _http_router: http_router,
        }
    }

    #[allow(dead_code)]
    pub async fn http_router(&mut self) -> &mut axum::Router {
        // wait till it's ready
        <axum::Router as tower::ServiceExt<axum::extract::Request<axum::body::Body>>>::ready(
            &mut self._http_router,
        )
        .await
        .unwrap();

        &mut self._http_router
    }
}
