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
    pub fn queue(task_queue_addr: impl ToString, poll_interval: u32, hd_index: u32) -> Self {
        TriggerRequest::Queue {
            task_queue_addr: task_queue_addr.to_string(),
            poll_interval,
            hd_index,
        }
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
            TriggerRequest::Queue {
                task_queue_addr,
                poll_interval,
                hd_index,
            } => {
                let task_queue_addr = match &self.state {
                    Some(state) => {
                        let chain_config: ChainConfig = state.config.cosmos_chain_config()?.into();
                        chain_config.parse_address(&task_queue_addr)?
                    }
                    None => Address::new_cosmos_string(&task_queue_addr, None)?,
                };

                let verifier_addr = match &self.state {
                    Some(state) if !state.is_mock_chain_client => {
                        let chain_config: ChainConfig = state.config.cosmos_chain_config()?.into();
                        query_verifier_addr(chain_config, &task_queue_addr).await?
                    }
                    _ => rand_address(),
                };

                let trigger = Trigger::queue(task_queue_addr, poll_interval);
                let submit = Some(Submit::verifier_tx(hd_index, verifier_addr));

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
                trigger: TriggerRequest::queue(rand_address(), 5, 0),
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
        fn make_service_req(addr: impl ToString) -> ServiceRequest {
            ServiceRequest {
                id: ID::new("test-name").unwrap(),
                digest: Digest::new(&[0; 32]).into(),
                trigger: TriggerRequest::queue(addr, 5, 0),
                permissions: Permissions::default(),
                envs: vec![],
                testable: Some(true),
            }
        }

        ServiceRequestParser::new(None)
            .parse(make_service_req("not-a-valid-addr"))
            .await
            .unwrap_err();
        ServiceRequestParser::new(None)
            .parse(make_service_req(rand_address().to_string()))
            .await
            .unwrap();
    }
}
