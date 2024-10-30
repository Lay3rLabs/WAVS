use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    apis::Trigger,
    http::{
        state::HttpState,
        types::app::{App, Permissions, Status},
    },
    Digest,
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAppsResponse {
    pub apps: Vec<App>,
    pub digests: Vec<Digest>,
}

#[axum::debug_handler]
pub async fn handle_list_services(State(_state): State<HttpState>) -> impl IntoResponse {
    // TEMPORARY PLACEHOLDER
    let response = ListAppsResponse {
        apps: vec![
            App {
                trigger: Trigger::Queue {
                    task_queue_addr: "layer18cpv22kxz9g7yljyvh309vd7al5qx40av3edkt".to_string(),
                    poll_interval: 1000,
                },
                name: "mock-service-1".to_string(),
                status: Some(Status::Active),
                digest: Digest::new(&[0; 32]),
                permissions: Permissions {},
                envs: Vec::new(),
                testable: None,
            },
            App {
                trigger: Trigger::Queue {
                    task_queue_addr: "layer18cpv22kxz9g7yljyvh309vd7al5qx40av3edkt".to_string(),
                    poll_interval: 1000,
                },
                name: "mock-service-2".to_string(),
                status: Some(Status::Active),
                digest: Digest::new(&[1; 32]),
                permissions: Permissions {},
                envs: Vec::new(),
                testable: None,
            },
        ],
        digests: vec![Digest::new(&[0; 32]), Digest::new(&[1; 32])],
    };

    Json(response).into_response()
}
