use std::collections::BTreeMap;

use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};

use crate::http::{error::HttpResult, state::HttpState};
use utils::types::{
    AddServiceRequest, AddServiceResponse, Component, Service, ServiceStatus, Submit, Workflow,
};

#[axum::debug_handler]
pub async fn handle_add_service(
    State(state): State<HttpState>,
    Json(req): Json<AddServiceRequest>,
) -> impl IntoResponse {
    match add_service_inner(state, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_service_inner(
    state: HttpState,
    req: AddServiceRequest,
) -> HttpResult<AddServiceResponse> {
    let service = ServiceRequestParser::new(Some(state.clone()))
        .parse(req)
        .await?;
    let service_id = service.id.clone();

    for workflow in service.workflows.values() {
        match &workflow.submit {
            Submit::None => {}
            Submit::EigenContract {
                chain_name,
                service_manager,
                max_gas: _,
            } => {
                let chain_config = state
                    .config
                    .chains
                    .eth
                    .get(&chain_name)
                    .context(format!("No chain config found for chain: {chain_name}"))?;
                if let Some(aggregator_endpoint) = &chain_config.aggregator_endpoint {
                    state
                        .http_client
                        .post(format!("{}/add-service", aggregator_endpoint))
                        .header("Content-Type", "application/json")
                        .json(
                            &utils::aggregator::AddAggregatorServiceRequest::EthTrigger {
                                service_manager_address: service_manager.clone().try_into()?,
                            },
                        )
                        .send()
                        .await?;
                }
            }
        }
    }

    state.dispatcher.add_service(service)?;

    Ok(AddServiceResponse { id: service_id })
}

#[allow(dead_code)]
struct ServiceRequestParser {
    state: Option<HttpState>,
}

impl ServiceRequestParser {
    fn new(state: Option<HttpState>) -> Self {
        Self { state }
    }

    async fn parse(&self, req: AddServiceRequest) -> anyhow::Result<Service> {
        let service_config = req.config.clone();
        let component_id = service_config.component_id;
        let workflow_id = service_config.workflow_id;
        let service_id = req.id;

        let component = Component {
            wasm: req.digest.into(),
            permissions: req.permissions,
        };

        let components = BTreeMap::from([(component_id.clone(), component)]);

        let workflows = BTreeMap::from([(
            workflow_id,
            Workflow {
                trigger: req.trigger,
                component: component_id,
                submit: req.submit,
            },
        )]);

        Ok(Service {
            id: service_id.clone(),
            name: service_id.to_string(),
            components,
            workflows,
            config: Some(req.config),
            status: ServiceStatus::Active,
            testable: req.testable.unwrap_or(false),
        })
    }
}

#[cfg(test)]
mod test {
    use crate::test_utils::address::rand_address_eth;
    use utils::{
        digest::Digest,
        types::{AddServiceRequest, ChainName, Permissions, ServiceConfig, Submit, Trigger},
        ServiceID,
    };

    use super::ServiceRequestParser;

    #[tokio::test]
    async fn add_service_validation() {
        let req = AddServiceRequest {
            id: ServiceID::new("test-name").unwrap(),
            digest: Digest::new(&[0; 32]).into(),
            trigger: Trigger::eth_contract_event(
                rand_address_eth(),
                ChainName::new("eth").unwrap(),
                [0; 32],
            ),
            permissions: Permissions::default(),
            testable: Some(true),
            submit: Submit::eigen_contract(
                ChainName::new("eth").unwrap(),
                rand_address_eth(),
                None,
            ),
            config: ServiceConfig::default(),
            wasm_url: None,
        };

        ServiceRequestParser::new(None).parse(req).await.unwrap();
    }
}
