use std::sync::Arc;

use axum::body::Body;
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use utils::{context::AppContext, storage::fs::FileStorage, telemetry::HttpMetrics};

use wavs::dispatcher::Dispatcher;

use super::{app::TestApp, mock_app::MockE2ETestRunner};

#[derive(Clone)]
pub struct TestHttpApp {
    pub inner: TestApp,
    pub ctx: AppContext,
    _temp_data_dir: Option<Arc<tempfile::TempDir>>,
    _http_router: axum::Router,
}

impl Default for TestHttpApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TestHttpApp {
    pub fn new() -> Self {
        let ctx = AppContext::new();

        let temp_data_dir = tempfile::tempdir().unwrap();
        Self::new_with_dispatcher(
            ctx.clone(),
            Arc::new(MockE2ETestRunner::create_dispatcher(
                ctx.clone(),
                &temp_data_dir,
            )),
            Some(temp_data_dir),
        )
    }

    pub fn new_with_dispatcher(
        ctx: AppContext,
        dispatcher: Arc<Dispatcher<FileStorage>>,
        temp_data_dir: Option<tempfile::TempDir>,
    ) -> Self {
        let inner = TestApp::new();

        let meter = opentelemetry::global::meter("wavs_test_metrics");
        let metrics = HttpMetrics::new(&meter);

        ctx.clone().rt.block_on(async move {
            let http_router = wavs::http::server::make_router(
                inner.config.as_ref().clone(),
                dispatcher,
                true,
                metrics,
            )
            .await
            .unwrap();

            Self {
                ctx,
                inner,
                _http_router: http_router,
                _temp_data_dir: temp_data_dir.map(Arc::new),
            }
        })
    }

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

pub async fn map_response<T: DeserializeOwned>(response: axum::http::Response<Body>) -> T {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}
