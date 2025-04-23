use super::*;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        config::handle_config,
        info::handle_info,
        packet::handle_packet,
        register_service::handle_register_service
    ),
    info(
        title = "WAVS Aggregator API",
        description = "API documentation for the WAVS aggregator service"
    )
)]
pub struct ApiDoc;
