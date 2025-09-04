use crate::{
    config::Config, dispatcher::Dispatcher,
    http::handlers::service::get::handle_get_service_by_hash, AppContext,
};
use axum::{
    extract::DefaultBodyLimit,
    middleware,
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

    // public routes
    let public = axum::Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .layer(OtelAxumLayer::default())
        .route("/config", get(handle_config))
        .route("/service", get(handle_get_service))
        .route(
            "/service-by-hash/{service_hash}",
            get(handle_get_service_by_hash),
        )
        .route("/app", get(handle_list_services))
        .route("/info", get(handle_info))
        .fallback(handle_not_found)
        .with_state(state.clone());

    // protected routes (POST/DELETE)
    let protected = axum::Router::new()
        .route("/service-key", post(handle_get_service_key))
        .route("/save-service", post(handle_save_service))
        .route("/app", post(handle_add_service))
        .route("/app", delete(handle_delete_service))
        .route("/add-chain", post(handle_add_chain))
        .route(
            "/upload",
            post(handle_upload_service).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        ) // 50MB limit
        .with_state(state);

    // apply bearer auth to protected routes if configured
    let mut router = public.merge(match &config.bearer_token {
        Some(token) => protected.layer(middleware::from_fn_with_state(
            (token.clone(), "wavs".to_string()),
            utils::http::verify_bearer_with_realm,
        )),
        None => protected,
    });

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
