use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;

use crate::http::{error::HttpResult, state::HttpState};
use wavs_types::AddServiceRequest;

#[axum::debug_handler]
pub async fn handle_add_service(
    State(state): State<HttpState>,
    Json(req): Json<AddServiceRequest>,
) -> impl IntoResponse {
    match add_service_inner(state, req).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_service_inner(state: HttpState, req: AddServiceRequest) -> HttpResult<()> {
    let AddServiceRequest { source } = req;

    match &source {
        wavs_types::ServiceMetadataSource::EthereumServiceManager {
            chain_name,
            contract_address,
        } => {
            let chain_config = state
                .config
                .chains
                .eth
                .get(&chain_name)
                .context(format!("No chain config found for chain: {chain_name}"))?;
            if let Some(aggregator_endpoint) = &chain_config.aggregator_endpoint {
                tracing::info!("SETTING AGGREAGATOR ENDPOINT: {aggregator_endpoint}");
                state
                    .http_client
                    .post(format!("{}/add-service", aggregator_endpoint))
                    .header("Content-Type", "application/json")
                    .json(&utils::aggregator::AddAggregatorServiceRequest::Ethereum {
                        address: *contract_address,
                    })
                    .send()
                    .await?;
            }
        }
    }

    state.dispatcher.add_service(source.clone()).await?;
    tracing::info!("Added service with source: {:?}", source);

    Ok(())
}
