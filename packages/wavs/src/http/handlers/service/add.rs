use std::collections::BTreeMap;

use anyhow::Context;
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    apis::{
        dispatcher::{
            Component, Permissions, Service, ServiceConfig, ServiceStatus, Submit, Workflow,
        },
        trigger::Trigger,
    },
    http::{error::HttpResult, state::HttpState, types::ShaDigest},
};
use utils::ServiceID;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddServiceRequest {
    #[serde(flatten)]
    pub service: ServiceRequest,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ServiceRequest {
    // on the wire, for v0.2, it's "name"
    // however, internally we repurpose this as the ID
    // so we'll just treat it as an ID for here, and keep "name" field for backwards compat
    #[serde(rename = "name")]
    pub id: ServiceID,
    pub digest: ShaDigest,
    pub trigger: Trigger,
    pub permissions: Permissions,
    pub config: ServiceConfig,
    pub testable: Option<bool>,
    pub submit: Submit,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddServiceResponse {
    pub id: ServiceID,
}

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
        .parse(req.service)
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
                    .get(chain_name)
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

    async fn parse(&self, req: ServiceRequest) -> anyhow::Result<Service> {
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
    use layer_climb::prelude::Address;

    use crate::{
        apis::{
            dispatcher::{Permissions, ServiceConfig, Submit},
            trigger::Trigger,
        },
        test_utils::address::rand_address_eth,
        Digest,
    };
    use utils::{types::ChainName, ServiceID};

    use super::{ServiceRequest, ServiceRequestParser};

    #[tokio::test]
    async fn add_service_validation() {
        fn make_service_req(addr: Address) -> ServiceRequest {
            ServiceRequest {
                id: ServiceID::new("test-name").unwrap(),
                digest: Digest::new(&[0; 32]).into(),
                trigger: Trigger::eth_contract_event(addr, ChainName::new("eth").unwrap(), [0; 32]),
                permissions: Permissions::default(),
                testable: Some(true),
                submit: Submit::eigen_contract(
                    ChainName::new("eth").unwrap(),
                    rand_address_eth(),
                    None,
                ),
                config: ServiceConfig::default(),
            }
        }

        ServiceRequestParser::new(None)
            .parse(make_service_req(rand_address_eth()))
            .await
            .unwrap();
    }
}
