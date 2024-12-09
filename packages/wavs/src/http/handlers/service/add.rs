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
    pub submit: Submit,
}

impl TriggerRequest {
    pub fn layer_queue(task_queue_addr: Address, poll_interval: u32, hd_index: u32) -> Self {
        TriggerRequest::LayerQueue {
            task_queue_addr,
            poll_interval,
            hd_index,
        }
    }

    pub fn eth_queue(task_queue_addr: Address, task_queue_erc1271: Address) -> Self {
        TriggerRequest::EthQueue {
            task_queue_addr,
            task_queue_erc1271,
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

#[allow(dead_code)]
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

        let trigger = match req.trigger {
            TriggerRequest::LayerQueue {
                task_queue_addr,
                poll_interval,
                hd_index: _,
            } => Trigger::layer_queue(task_queue_addr, poll_interval),

            TriggerRequest::EthQueue {
                task_queue_addr,
                task_queue_erc1271,
            } => Trigger::eth_queue(task_queue_addr, task_queue_erc1271),
        };

        let workflows = BTreeMap::from([(
            workflow_id,
            Workflow {
                trigger,
                component: component_id,
                submit: Some(req.submit),
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

#[cfg(test)]
mod test {
    use layer_climb::prelude::Address;
    use serde::{Deserialize, Serialize};

    use crate::{
        apis::{
            dispatcher::{Permissions, ServiceStatus, Submit},
            ID,
        },
        http::{handlers::service::add::TriggerRequest, types::ShaDigest},
        test_utils::address::rand_address_eth,
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
    async fn add_service_validation() {
        fn make_service_req(addr: Address, erc1271: Address) -> ServiceRequest {
            ServiceRequest {
                id: ID::new("test-name").unwrap(),
                digest: Digest::new(&[0; 32]).into(),
                trigger: TriggerRequest::eth_queue(addr, erc1271),
                permissions: Permissions::default(),
                envs: vec![],
                testable: Some(true),
                submit: Submit::eth_aggregator_tx(),
            }
        }

        ServiceRequestParser::new(None)
            .parse(make_service_req(rand_address_eth(), rand_address_eth()))
            .await
            .unwrap();
    }
}
