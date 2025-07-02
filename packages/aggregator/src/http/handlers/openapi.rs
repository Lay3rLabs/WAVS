use super::*;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        config::handle_config,
        info::handle_info,
        packet::handle_packet,
    ),
    info(
        title = "WAVS Aggregator API",
        description = "API documentation for the WAVS aggregator service"
    )
)]
pub struct ApiDoc;
