use crate::{config::Config, AppContext};
use axum::routing::{get, post};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use wildmatch::WildMatch;

use super::{
    handlers::{
        handle_config, handle_info, handle_not_found,
        service::{add_payload::handle_add_payload, add_service::handle_add_service},
    },
    state::HttpState,
};

// this is called from main
pub fn start(
    ctx: AppContext,
    config: Config,
) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {
    let mut shutdown_signal = ctx.get_kill_receiver();
    let handle = ctx.rt.spawn(async move {
        let (host, port) = (config.host.clone(), config.port);

        let router = make_router(config).await?;

        let listener = tokio::net::TcpListener::bind(&format!("{}:{}", host, port)).await?;

        tracing::info!("Http server starting on: {}", listener.local_addr()?);

        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown_signal.recv().await.ok();
                tracing::debug!("Http server shutting down");
            })
            .await?;

        anyhow::Ok(())
    });

    Ok(handle)
}

// this is called from main and tests
pub async fn make_router(config: Config) -> anyhow::Result<axum::Router> {
    let state = HttpState::new(config.clone())?;

    // build our application with a single route
    let mut router = axum::Router::new()
        .layer(TraceLayer::new_for_http())
        .route("/config", get(handle_config))
        .route("/info", get(handle_info))
        .route("/add-payload", post(handle_add_payload))
        .route("/add-service", post(handle_add_service))
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
