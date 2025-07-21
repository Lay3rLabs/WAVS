use crate::{config::Config, http::handlers::handle_register_service, AppContext};
use axum::routing::{get, post};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::instrument;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use wildmatch::WildMatch;

use super::{
    handlers::{
        handle_config, handle_info, handle_not_found, handle_packet, handle_upload, ApiDoc,
    },
    state::HttpState,
};

// this is called from main
#[instrument(level = "info", skip(ctx, config))]
pub fn start(ctx: AppContext, config: Config) -> anyhow::Result<()> {
    let mut shutdown_signal = ctx.get_kill_receiver();
    ctx.rt.block_on(async move {
        let (host, port) = (config.host.clone(), config.port);

        let router = make_router(config).await?;

        let listener = tokio::net::TcpListener::bind(&format!("{}:{}", host, port)).await?;

        tracing::info!("HTTP server starting on: {}", listener.local_addr()?);

        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown_signal.recv().await.ok();
                tracing::info!("HTTP server shutting down");
            })
            .await?;

        anyhow::Ok(())
    })
}

// this is called from main and tests
pub async fn make_router(config: Config) -> anyhow::Result<axum::Router> {
    tracing::info!("Creating file storage at: {:?}", config.data);
    let file_storage = utils::storage::fs::FileStorage::new(&config.data)?;
    let ca_storage = std::sync::Arc::new(file_storage);
    tracing::info!("Creating HttpState with engine");
    let state = HttpState::new_with_engine(config.clone(), ca_storage)?;
    tracing::info!(
        "HttpState created successfully with engine: {}",
        state.aggregator_engine.is_some()
    );

    // build our application with a single route
    let mut router = axum::Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .route("/config", get(handle_config))
        .route("/info", get(handle_info))
        .route("/packet", post(handle_packet))
        .route("/register-service", post(handle_register_service))
        .route("/upload", post(handle_upload))
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
