use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        super::config::handle_config,
        super::info::handle_info,
        super::packet::handle_packet,
        super::register_service::handle_register_service
    ),
    info(
        title = "WAVS Aggregator API",
        description = "API documentation for the WAVS aggregator service"
    )
)]
pub struct ApiDoc;
