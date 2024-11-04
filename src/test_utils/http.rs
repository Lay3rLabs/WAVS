use std::sync::Arc;

use crate::{
    apis::{engine::Engine, submission::Submission, trigger::TriggerManager},
    dispatcher::Dispatcher,
    engine::identity::IdentityEngine,
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
        let engine = IdentityEngine::new();
        let submission = MockSubmission::new();
        let storage_path = tempfile::NamedTempFile::new().unwrap();

        let dispatcher =
            Arc::new(Dispatcher::new(trigger_manager, engine, submission, storage_path).unwrap());

        Self::new_with_dispatcher(dispatcher).await
    }

    pub async fn new_with_dispatcher<T, E, S>(dispatcher: Arc<Dispatcher<T, E, S>>) -> Self
    where
        T: TriggerManager + 'static,
        E: Engine + 'static,
        S: Submission + 'static,
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
