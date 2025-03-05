use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};
use reqwest::StatusCode;

use crate::http::{error::HttpResult, state::HttpState};
use wavs_types::{AddServiceRequest, Submit};

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
    let AddServiceRequest { service } = req;

    for workflow in service.workflows.values() {
        match &workflow.submit {
            Submit::None => {}
            Submit::EthereumContract {
                chain_name,
                address,
                max_gas: _,
            } => {
                let chain_config = state
                    .config
                    .chains
                    .eth
                    .get(chain_name)
                    .context(format!("No chain config found for chain: {chain_name}"))?;
                if let Some(aggregator_endpoint) = &chain_config.aggregator_endpoint {
                    state
                        .http_client
                        .post(format!("{}/add-service", aggregator_endpoint))
                        .header("Content-Type", "application/json")
                        .json(
                            &utils::aggregator::AddAggregatorServiceRequest::EthTrigger {
                                address: *address,
                            },
                        )
                        .send()
                        .await?;
                }
            }
        }
    }

    state.dispatcher.add_service(service)?;

    Ok(())
}
