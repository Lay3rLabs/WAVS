use std::ops::Bound;

use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    apis::{
        dispatcher::{Permissions, ServiceStatus, Submit},
        ServiceID,
    },
    http::{
        error::HttpResult,
        state::HttpState,
        types::{ShaDigest, TriggerResponse},
    },
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListServicesResponse {
    // on the wire, for v0.2, it's "apps"
    // however, internally we are calling these "services"
    // so we'll just treat it as a service here, and keep "apps" field for backwards compat
    #[serde(rename = "apps")]
    pub services: Vec<ServiceResponse>,
    pub digests: Vec<ShaDigest>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceResponse {
    /// In 0.2 this is called "name"
    /// This is the ID of the service
    #[serde(rename = "name")]
    pub id: ServiceID,
    pub status: ServiceStatus,
    pub digest: ShaDigest,
    // for 0.3, it might be nice to make this just Trigger, but the address type breaks backwards compat
    pub trigger: TriggerResponse,
    pub permissions: Permissions,
    pub testable: Option<bool>,
}

#[axum::debug_handler]
pub async fn handle_list_services(State(state): State<HttpState>) -> impl IntoResponse {
    match list_services_inner(&state).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn list_services_inner(state: &HttpState) -> HttpResult<ListServicesResponse> {
    let services_list = state
        .dispatcher
        .list_services(Bound::Unbounded, Bound::Unbounded)?;

    let mut services = Vec::with_capacity(services_list.len());

    // for backwards compatibility, we do some funky things here
    // it will be nicer in 0.3
    for service in services_list {
        for component in service.components.values() {
            services.push(ServiceResponse {
                digest: component.wasm.clone().into(),
                permissions: component.permissions.clone(),
                status: service.status,
                id: service.id.clone(),
                // just first workflow for now
                trigger: match service.workflows.values().next() {
                    None => return Err(anyhow::anyhow!("No workflows found").into()),
                    Some(w) => {
                        let hd_index = w
                            .submit
                            .as_ref()
                            .map(|s| match s {
                                Submit::LayerVerifierTx { hd_index, .. } => *hd_index,
                                Submit::EthAggregatorTx { .. } => 0,
                                Submit::EthSignedMessage { hd_index, .. } => *hd_index,
                            })
                            .unwrap_or(0);

                        w.trigger.clone().into_response(hd_index)
                    }
                },
                testable: Some(service.testable),
            });
        }
    }

    let digests = state.dispatcher.list_component_digests()?;
    let digests = digests.into_iter().map(Into::into).collect();

    Ok(ListServicesResponse { services, digests })
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::{
        apis::dispatcher::{Permissions, ServiceStatus},
        http::{
            handlers::service::list::ListServicesResponse,
            types::{ShaDigest, TriggerResponse},
        },
        test_utils::address::rand_address_eth,
        Digest,
    };

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct OldListAppsResponse {
        pub apps: Vec<OldApp>,
        pub digests: Vec<ShaDigest>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct OldApp {
        pub name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<ServiceStatus>,
        pub digest: ShaDigest,
        pub trigger: TriggerResponse,
        pub permissions: Permissions,
        pub testable: Option<bool>,
    }

    #[tokio::test]
    async fn list_services_backwards_compat() {
        let old = OldListAppsResponse {
            apps: vec![
                OldApp {
                    name: "test-name-1".to_string(),
                    status: Some(ServiceStatus::Active),
                    digest: Digest::new(&[0; 32]).into(),
                    trigger: TriggerResponse::eth_event(rand_address_eth()),
                    permissions: Permissions::default(),
                    testable: Some(true),
                },
                OldApp {
                    name: "test-name-2".to_string(),
                    status: Some(ServiceStatus::Active),
                    digest: Digest::new(&[0; 32]).into(),
                    trigger: TriggerResponse::eth_event(rand_address_eth()),
                    permissions: Permissions::default(),
                    testable: Some(true),
                },
                OldApp {
                    name: "test-name-3".to_string(),
                    status: Some(ServiceStatus::Active),
                    digest: Digest::new(&[0; 32]).into(),
                    trigger: TriggerResponse::eth_event(rand_address_eth()),
                    permissions: Permissions::default(),
                    testable: Some(true),
                },
            ],
            digests: vec![Digest::new(&[0; 32]).into(), Digest::new(&[0; 32]).into()],
        };

        let old_str = serde_json::to_string_pretty(&old).unwrap();

        let updated: ListServicesResponse = serde_json::from_str(&old_str).unwrap();

        let updated_str = serde_json::to_string(&updated).unwrap();

        let old_roundtrip: OldListAppsResponse = serde_json::from_str(&updated_str).unwrap();

        assert_eq!(old, old_roundtrip);
    }
}
