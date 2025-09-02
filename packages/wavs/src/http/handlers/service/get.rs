use std::str::FromStr;

use crate::http::{error::HttpResult, state::HttpState};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use wavs_types::{ChainName, ServiceDigest, ServiceID, ServiceManager};

#[derive(Deserialize)]
pub struct GetServiceParams {
    pub chain_name: String,
    pub address: String,
}

#[utoipa::path(
    get,
    path = "/service",
    params(
        ("chain_name" = String, Query, description = "Name of the chain"),
        ("address" = String, Query, description = "Service contract address")
    ),
    responses(
        (status = 200, description = "Service found", body = wavs_types::Service),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Retrieves detailed information about a specific service"
)]
#[axum::debug_handler]
pub async fn handle_get_service(
    State(state): State<HttpState>,
    Query(params): Query<GetServiceParams>,
) -> impl IntoResponse {
    let result = async move {
        let chain_name = ChainName::new(params.chain_name)?;
        let address = params.address.parse::<alloy_primitives::Address>()?;
        let service_manager = ServiceManager::Evm {
            chain_name,
            address,
        };
        get_service_inner(&state, service_manager).await
    }
    .await;

    match result {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn get_service_inner(
    state: &HttpState,
    service_manager: ServiceManager,
) -> HttpResult<wavs_types::Service> {
    Ok(state.load_service(&ServiceID::from(&service_manager))?)
}

#[utoipa::path(
    get,
    path = "/service-by-hash/{service_hash}",
    params(
        ("service_hash" = String, Path, description = "Unique hash of the service")
    ),
    responses(
        (status = 200, description = "Service found", body = wavs_types::Service),
        (status = 404, description = "Service not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Retrieves detailed information about a specific service by its Hash (for testing only)"
)]
#[axum::debug_handler]
pub async fn handle_get_service_by_hash(
    State(state): State<HttpState>,
    axum::extract::Path(service_hash): axum::extract::Path<String>,
) -> impl IntoResponse {
    match get_service_inner_hash(&state, service_hash).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn get_service_inner_hash(
    state: &HttpState,
    service_hash: String,
) -> HttpResult<wavs_types::Service> {
    let service_hash = ServiceDigest::from_str(&service_hash)?;

    Ok(state.load_service_by_hash(&service_hash)?)
}
