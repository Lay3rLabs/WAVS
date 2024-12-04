use std::collections::BTreeMap;

use axum::{extract::State, response::IntoResponse, Json};
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    apis::{
        dispatcher::{Component, Permissions, Service, ServiceStatus, Submit, Workflow},
        Trigger, ID,
    },
    http::{
        error::HttpResult,
        state::HttpState,
        types::{ShaDigest, TriggerRequest},
    },
    test_utils::address::rand_address,
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddServiceRequest {
    #[serde(flatten)]
    pub service: ServiceRequest,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRequest {
    // on the wire, for v0.2, it's "name"
    // however, internally we repurpose this as the ID
    // so we'll just treat it as an ID for here, and keep "name" field for backwards compat
    #[serde(rename = "name")]
    pub id: ID,
    pub digest: ShaDigest,
    pub trigger: TriggerRequest,
    pub permissions: Permissions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub envs: Vec<(String, String)>,
    pub testable: Option<bool>,
}

impl TriggerRequest {
    pub fn layer_queue(task_queue_addr: Address, poll_interval: u32, hd_index: u32) -> Self {
        TriggerRequest::LayerQueue {
            task_queue_addr,
            poll_interval,
            hd_index,
        }
    }

    pub fn eth_queue(task_queue_addr: Address) -> Self {
        TriggerRequest::EthQueue { task_queue_addr }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AddServiceResponse {
    // on the wire, for v0.2, it's "name"
    // however, internally we repurpose this as the ID
    // so we'll just treat it as an ID for here, and keep "name" field for backwards compat
    #[serde(rename = "name")]
    pub id: ID,
    // TODO for 0.3, not sure why this is needed, it's always "Active"
    pub status: ServiceStatus,
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

    state.dispatcher.add_service(service)?;

    Ok(AddServiceResponse {
        id: service_id,
        status: ServiceStatus::Active,
    })
}

struct ServiceRequestParser {
    state: Option<HttpState>,
}

impl ServiceRequestParser {
    fn new(state: Option<HttpState>) -> Self {
        Self { state }
    }

    async fn parse(&self, req: ServiceRequest) -> anyhow::Result<Service> {
        let component_id = ID::new("default")?;
        let workflow_id = ID::new("default")?;
        let service_id = req.id;

        let component = Component {
            wasm: req.digest.into(),
            permissions: req.permissions,
            env: req.envs,
        };

        let components = BTreeMap::from([(component_id.clone(), component)]);

        let (trigger, submit) = match req.trigger {
            TriggerRequest::LayerQueue {
                task_queue_addr,
                poll_interval,
                hd_index,
            } => {
                let verifier_addr = match &self.state {
                    Some(state) if !state.is_mock_chain_client => {
                        let chain_config: ChainConfig = state.config.layer_chain_config()?.into();
                        query_verifier_addr(chain_config, &task_queue_addr).await?
                    }
                    _ => rand_address(),
                };

                let trigger = Trigger::layer_queue(task_queue_addr, poll_interval);
                let submit = Some(Submit::layer_verifier_tx(hd_index, verifier_addr));

                (trigger, submit)
            }

            TriggerRequest::EthQueue { task_queue_addr } => {
                let trigger = Trigger::eth_queue(task_queue_addr);
                let submit = Some(Submit::eth_aggregator_tx());

                (trigger, submit)
            }
        };

        let workflows = BTreeMap::from([(
            workflow_id,
            Workflow {
                trigger,
                component: component_id,
                submit,
            },
        )]);

        Ok(Service {
            id: service_id.clone(),
            name: service_id.to_string(),
            components,
            workflows,
            status: ServiceStatus::Active,
            testable: req.testable.unwrap_or(false),
        })
    }
}

async fn query_verifier_addr(
    chain_config: ChainConfig,
    task_queue_addr: &Address,
) -> anyhow::Result<Address> {
    let query_client = QueryClient::new(chain_config).await?;

    let resp: lavs_apis::tasks::ConfigResponse = query_client
        .contract_smart(
            task_queue_addr,
            &lavs_apis::tasks::QueryMsg::Custom(lavs_apis::tasks::CustomQueryMsg::Config {}),
        )
        .await?;

    query_client.chain_config.parse_address(&resp.verifier)
}

#[cfg(test)]
mod test {
    use layer_climb::prelude::Address;
    use serde::{Deserialize, Serialize};

    use crate::{
        apis::{
            dispatcher::{Permissions, ServiceStatus},
            ID,
        },
        http::{
            handlers::service::add::{AddServiceRequest, TriggerRequest},
            types::ShaDigest,
        },
        test_utils::address::rand_address,
        Digest,
    };

    use super::{ServiceRequest, ServiceRequestParser};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct OldRegisterAppRequest {
        #[serde(flatten)]
        pub app: OldApp,

        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wasm_url: Option<String>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct OldApp {
        pub name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<ServiceStatus>,
        pub digest: ShaDigest,
        pub trigger: TriggerRequest,
        pub permissions: Permissions,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub envs: Vec<(String, String)>,
        pub testable: Option<bool>,
    }

    #[tokio::test]
    async fn add_service_backwards_compat() {
        let old = OldRegisterAppRequest {
            wasm_url: None,
            app: OldApp {
                name: "test-name".to_string(),
                status: None,
                digest: Digest::new(&[0; 32]).into(),
                trigger: TriggerRequest::eth_queue(rand_address()),
                permissions: Permissions::default(),
                envs: vec![],
                testable: Some(true),
            },
        };

        let old_str = serde_json::to_string(&old).unwrap();

        let updated: AddServiceRequest = serde_json::from_str(&old_str).unwrap();

        ServiceRequestParser::new(None)
            .parse(updated.service.clone())
            .await
            .unwrap();

        let updated_str = serde_json::to_string(&updated).unwrap();

        let old_roundtrip: OldRegisterAppRequest = serde_json::from_str(&updated_str).unwrap();

        assert_eq!(old, old_roundtrip);
    }

    #[tokio::test]
    async fn add_service_validation() {
        fn make_service_req(addr: Address) -> ServiceRequest {
            ServiceRequest {
                id: ID::new("test-name").unwrap(),
                digest: Digest::new(&[0; 32]).into(),
                trigger: TriggerRequest::eth_queue(addr),
                permissions: Permissions::default(),
                envs: vec![],
                testable: Some(true),
            }
        }

        ServiceRequestParser::new(None)
            .parse(make_service_req(rand_address()))
            .await
            .unwrap();
    }
}
