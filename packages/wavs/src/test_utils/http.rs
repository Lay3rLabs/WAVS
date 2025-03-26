use std::sync::Arc;

use axum::body::Body;
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;

use crate::{
    apis::{service::ServiceCache, submission::Submission, trigger::TriggerManager},
    dispatcher::Dispatcher,
    engine::{
        identity::IdentityEngine,
        runner::{EngineRunner, SingleEngineRunner},
    },
    service::mock::MockServiceCache,
    submission::mock::MockSubmission,
    triggers::mock::MockTriggerManagerVec,
};

use super::app::TestApp;

#[derive(Clone)]
pub struct TestHttpApp {
    pub inner: TestApp,
    _http_router: axum::Router,
}

impl TestHttpApp {
    pub async fn new() -> Self {
        let trigger_manager = MockTriggerManagerVec::new();
        let engine = SingleEngineRunner::new(IdentityEngine::new());
        let submission = MockSubmission::new();
        let service_manager = MockServiceCache::new();
        let storage_path = tempfile::NamedTempFile::new().unwrap();

        let dispatcher = Arc::new(
            Dispatcher::new(
                trigger_manager,
                engine,
                submission,
                service_manager,
                storage_path,
            )
            .unwrap(),
        );

        Self::new_with_dispatcher(dispatcher).await
    }

    pub async fn new_with_dispatcher<T, E, S, C>(dispatcher: Arc<Dispatcher<T, E, S, C>>) -> Self
    where
        T: TriggerManager + 'static,
        E: EngineRunner + 'static,
        S: Submission + 'static,
        C: ServiceCache + 'static,
    {
        let inner = TestApp::new().await;

        let http_router =
            crate::http::server::make_router(inner.config.as_ref().clone(), dispatcher, true)
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

pub async fn map_response<T: DeserializeOwned>(response: axum::http::Response<Body>) -> T {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}
