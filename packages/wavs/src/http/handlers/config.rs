use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::Credential;

use crate::{config::Config, http::state::HttpState};

const REDACTED: &str = "redacted";

#[utoipa::path(
    get,
    path = "/dev/config",
    responses(
        (status = 200, description = "Successfully retrieved configuration", body = Config),
        (status = 500, description = "Internal server error occurred while fetching configuration")
    ),
    description = "Returns the current configuration settings for WAVS (credentials are redacted)"
)]
#[axum::debug_handler]
pub async fn handle_config(State(state): State<HttpState>) -> impl IntoResponse {
    let mut config = state.config.clone();

    // Redact sensitive credentials
    fn redact(cred: &mut Option<Credential>) {
        if cred.is_some() {
            *cred = Some(Credential::new(REDACTED.to_string()));
        }
    }

    redact(&mut config.signing_mnemonic);
    redact(&mut config.aggregator_cosmos_credential);
    redact(&mut config.aggregator_evm_credential);
    redact(&mut config.bearer_token);

    Json(config).into_response()
}
