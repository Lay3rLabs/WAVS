use super::service::*;
use super::*;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        config::handle_config,
        get::handle_get_service,
        key::handle_get_service_signer,
        save::handle_save_service,
        list::handle_list_services,
        add::handle_add_service,
        delete::handle_delete_service,
        info::handle_info,
        upload::handle_upload_component
    ),
    info(
        title = "WAVS API",
        description = "API documentation for the WAVS service"
    )
)]
pub struct ApiDoc;
