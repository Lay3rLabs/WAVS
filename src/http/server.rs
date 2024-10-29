use crate::{
    apis::dispatcher::DispatchManager,
    config::Config,
    dispatcher::core::{CoreDispatcher, DispatcherError},
};
use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use wildmatch::WildMatch;

use super::{
    handlers::{
        handle_add_service, handle_config, handle_delete_service, handle_info,
        handle_list_services, handle_not_found, handle_test_service, handle_upload_service,
    },
    state::HttpState,
};

pub fn start(dispatcher: Arc<CoreDispatcher>) -> anyhow::Result<()> {
    // The server runs within the tokio runtime
    dispatcher.clone().async_runtime.block_on(async move {
        let (host, port) = (dispatcher.config.host.clone(), dispatcher.config.port);

        let router = make_router(dispatcher).await?;

        let listener = tokio::net::TcpListener::bind(&format!("{}:{}", host, port)).await?;

        tracing::info!("Http server starting on: {}", listener.local_addr()?);

        axum::serve(listener, router).await?;

        anyhow::Ok(())
    })?;

    Ok(())
}

pub async fn make_router<D: DispatchManager<Error = DispatcherError> + 'static>(
    dispatcher: Arc<D>,
) -> anyhow::Result<axum::Router> {
    let state = HttpState::new(dispatcher.clone()).await?;

    // build our application with a single route
    let mut router = axum::Router::new()
        .layer(TraceLayer::new_for_http())
        .route("/config", get(handle_config))
        .route("/app", get(handle_list_services))
        .route("/app", post(handle_add_service))
        .route("/app", delete(handle_delete_service))
        .route("/info", get(handle_info))
        .route("/test", post(handle_test_service))
        .route(
            "/upload",
            post(handle_upload_service).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        ) // 50MB limit
        .fallback(handle_not_found)
        .with_state(state);

    if let Some(cors) = cors_layer(dispatcher.config()) {
        router = router.layer(cors);
    }

    Ok(router)
}

fn cors_layer(config: &Config) -> Option<CorsLayer> {
    if config.cors_allowed_origins.is_empty() {
        None
    } else {
        let allowed_origins: Vec<WildMatch> = config
            .cors_allowed_origins
            .iter()
            .map(|s| WildMatch::new(s))
            .collect();

        Some(
            CorsLayer::new()
                // using a predicate so we have more flexibility over wildcard patterns
                .allow_origin(tower_http::cors::AllowOrigin::predicate(
                    move |origin, _parts| {
                        origin
                            .to_str()
                            .map(|origin| {
                                allowed_origins
                                    .iter()
                                    .any(|allowed_origin| allowed_origin.matches(origin))
                            })
                            .unwrap_or(false)
                    },
                ))
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
    }
}
