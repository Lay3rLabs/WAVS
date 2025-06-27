use crate::{config::Config, dispatcher::Dispatcher, AppContext};
use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
};
use axum_tracing_opentelemetry::middleware::OtelAxumLayer;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use utils::{storage::fs::FileStorage, telemetry::HttpMetrics};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use wildmatch::WildMatch;

use super::{
    handlers::{
        handle_add_chain, handle_add_service, handle_config, handle_delete_service, handle_info,
        handle_list_services, handle_not_found, handle_upload_service,
        openapi::ApiDoc,
        service::{
            get::handle_get_service, key::handle_get_service_key, save::handle_save_service,
        },
    },
    state::HttpState,
};

// this is called from main, takes a file-based Dispatcher
pub fn start(
    ctx: AppContext,
    config: Config,
    dispatcher: Arc<Dispatcher<FileStorage>>,
    metrics: HttpMetrics,
) -> anyhow::Result<()> {
    // The server runs within the tokio runtime
    ctx.rt.clone().block_on(async move {
        let (host, port) = (config.host.clone(), config.port);

        let mut shutdown_signal = ctx.get_kill_receiver();

        let router = make_router(config, dispatcher, false, metrics).await?;

        let listener = tokio::net::TcpListener::bind(&format!("{}:{}", host, port)).await?;

        tracing::info!("Http server starting on: {}", listener.local_addr()?);

        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown_signal.recv().await.ok();

                tracing::debug!("Http server shutting down");
            })
            .await?;

        anyhow::Ok(())
    })?;

    Ok(())
}

// this is called from main and tests
pub async fn make_router(
    config: Config,
    dispatcher: Arc<Dispatcher<FileStorage>>,
    is_mock_chain_client: bool,
    metrics: HttpMetrics,
) -> anyhow::Result<axum::Router> {
    let state = HttpState::new(config.clone(), dispatcher, is_mock_chain_client, metrics).await?;

    // build our application with a single route
    let mut router = axum::Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .layer(OtelAxumLayer::default())
        .route("/config", get(handle_config))
        .route("/service/{service_id}", get(handle_get_service))
        .route("/service-key/{service_id}", get(handle_get_service_key))
        .route("/save-service", post(handle_save_service))
        .route("/app", get(handle_list_services))
        .route("/app", post(handle_add_service))
        .route("/app", delete(handle_delete_service))
        .route("/add-chain", post(handle_add_chain))
        .route("/info", get(handle_info))
        .route(
            "/upload",
            post(handle_upload_service).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        ) // 50MB limit
        .fallback(handle_not_found)
        .with_state(state);

    if let Some(cors) = cors_layer(&config) {
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
