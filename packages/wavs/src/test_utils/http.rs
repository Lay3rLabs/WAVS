use std::sync::Arc;

use axum::body::Body;
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use utils::{
    config::ChainConfigs,
    telemetry::{DispatcherMetrics, HttpMetrics, Metrics},
};

use crate::{
    apis::submission::Submission,
    dispatcher::Dispatcher,
    engine::{
        identity::IdentityEngine,
        runner::{EngineRunner, SingleEngineRunner},
    },
    submission::mock::MockSubmission,
    trigger_manager::TriggerManager,
};

use super::app::TestApp;

#[derive(Clone)]
pub struct TestHttpApp {
    pub inner: TestApp,
    _http_router: axum::Router,
}

impl TestHttpApp {
    pub async fn new() -> Self {
        let config = crate::config::Config::default();
        let meter = opentelemetry::global::meter("wavs_metrics");
        let metrics = Metrics::new(&meter);
        let trigger_manager = TriggerManager::new(&config, metrics.wavs.trigger).unwrap();
        let engine = SingleEngineRunner::new(IdentityEngine::new());
        let submission = MockSubmission::new();
        let storage_path = tempfile::NamedTempFile::new().unwrap();
        let metrics = DispatcherMetrics::new(&opentelemetry::global::meter("trigger-test-metrics"));

        let dispatcher = Arc::new(
            Dispatcher::new(
                trigger_manager,
                engine,
                submission,
                ChainConfigs::default(),
                storage_path,
                metrics,
                "https://ipfs.io/ipfs/".to_string(),
            )
            .unwrap(),
        );

        Self::new_with_dispatcher(dispatcher).await
    }

    pub async fn new_with_dispatcher<E, S>(dispatcher: Arc<Dispatcher<E, S>>) -> Self
    where
        E: EngineRunner + 'static,
        S: Submission + 'static,
    {
        let inner = TestApp::new().await;

        let meter = opentelemetry::global::meter("wavs_test_metrics");
        let metrics = HttpMetrics::new(&meter);

        let http_router = crate::http::server::make_router(
            inner.config.as_ref().clone(),
            dispatcher,
            true,
            metrics,
        )
        .await
        .unwrap();

        Self {
            inner,
            _http_router: http_router,
        }
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
