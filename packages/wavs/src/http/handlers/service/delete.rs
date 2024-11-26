use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    apis::ID,
    http::{error::HttpResult, state::HttpState},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteServices {
    // on the wire, for v0.2, it's "apps"
    // however, internally we repurpose this as the ID
    // so we'll just treat it as an ID for here, and keep "apps" field for backwards compat
    #[serde(rename = "apps")]
    pub service_ids: Vec<ID>,
}

#[axum::debug_handler]
pub async fn handle_delete_service(
    State(state): State<HttpState>,
    Json(req): Json<DeleteServices>,
) -> impl IntoResponse {
    match delete_service_inner(state, req.service_ids).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn delete_service_inner(state: HttpState, service_ids: Vec<ID>) -> HttpResult<()> {
    for id in service_ids {
        state.dispatcher.remove_service(id)?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::DeleteServices;

    #[test]
    fn add_service_backwards_compat() {
        let old = r#"{"apps":["test"]}"#;
        let updated: DeleteServices = serde_json::from_str(old).unwrap();
        assert_eq!(updated.service_ids[0].to_string(), "test");
    }
}
