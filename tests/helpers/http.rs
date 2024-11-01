use std::sync::Arc;

use wasmatic::dispatcher::MockDispatcherBuilder;

use super::app::TestApp;

#[derive(Clone)]
pub struct TestHttpApp {
    pub inner: TestApp,
    _http_router: axum::Router,
}

impl TestHttpApp {
    pub async fn new() -> Self {
        Self::new_with_dispatcher(MockDispatcherBuilder::new()).await
    }

    pub async fn new_with_dispatcher(dispatcher_builder: MockDispatcherBuilder) -> Self {
        let inner = TestApp::new().await;

        let dispatcher = Arc::new(dispatcher_builder.build());

        let http_router =
            wasmatic::http::server::make_router(inner.config.as_ref().clone(), dispatcher, true)
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
