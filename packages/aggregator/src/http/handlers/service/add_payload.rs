use alloy::primitives::{Address, PrimitiveSignature, U256};
use anyhow::anyhow;
use axum::{extract::State, response::IntoResponse, Json};
use utils::{
    aggregator::{AggregateAvsRequest, AggregateAvsResponse},
    avs_client::{
        layer_service_manager::LayerServiceManager, stake_registry::ECDSAStakeRegistry,
        SignedPayload,
    },
    eigen_client::solidity_types::{
        misc::{AVSDirectory, IAVSDirectory::OperatorAVSRegistrationStatus},
        BoxSigningProvider,
    },
};

use crate::http::{error::HttpResult, state::HttpState};

#[axum::debug_handler]
pub async fn handle_add_payload(
    State(state): State<HttpState>,
    Json(req): Json<AggregateAvsRequest>,
) -> impl IntoResponse {
    let resp = match req {
        AggregateAvsRequest::EigenContract {
            signed_payload,
            service_manager_address,
        } => add_payload(state, signed_payload, service_manager_address).await,
    };

    match resp {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn add_payload(
    state: HttpState,
    signed_payload: SignedPayload,
    service_manager_address: Address,
) -> HttpResult<AggregateAvsResponse> {
    let eth_client = state.config.signing_client().await?;

    check_operator(
        service_manager_address,
        signed_payload.clone(),
        &eth_client.provider,
    )
    .await?;

    let mut queue = state.load_all_payloads(service_manager_address)?;

    queue.push(signed_payload);

    let count = queue.len();

    let resp = if count >= state.config.tasks_quorum as usize {
        // TODO: decide how to batch it. It seems like a complex topic with many options
        // Current options require something from node, extra contract dependency or uncertainty
        // Options:
        // - Use `Multicall` contract, tracking issue: https://github.com/alloy-rs/alloy/issues/328,
        //      non-official implementation: https://crates.io/crates/alloy-multicall (there is no send, how to even use it)
        // - trace_call_many, check note: https://docs.rs/alloy/0.8.0/alloy/providers/ext/trait.TraceApi.html#tymethod.trace_call_many
        // - Use eip-7702, doubt it's possible to do exactly what we're trying to achieve here
        // ✅(currently implemented) Add respond many on AVS endpoint

        // Send batch txs
        // let hello_world_service =
        //     hello_world::HelloWorldServiceManager::new(req.service, &eth_client.provider);

        let payloads = queue
            .drain(..)
            .map(|x| x.into_submission_abi())
            .collect::<Vec<_>>();

        let pending_tx = LayerServiceManager::new(service_manager_address, &eth_client.provider)
            .addPayloadMulti(payloads)
            .send()
            .await?;
        let tx_hash = pending_tx.tx_hash();
        tracing::debug!("Sent transaction: {}", tx_hash);

        let tx_hash = pending_tx.watch().await?;
        tracing::debug!("Transactions included in a block");

        AggregateAvsResponse::Sent { count, tx_hash }
    } else {
        AggregateAvsResponse::Aggregated { count }
    };

    state.save_all_payloads(service_manager_address, queue)?;

    Ok(resp)
}

pub async fn check_operator(
    service_manager_address: Address,
    signed_payload: SignedPayload,
    provider: &BoxSigningProvider,
) -> HttpResult<()> {
    let service_manager_contract = LayerServiceManager::new(service_manager_address, provider);
    let operator = signed_payload.operator;

    // Check Operator is registered
    let avs_directory_address = service_manager_contract.avsDirectory().call().await?._0;
    let avs_directory = AVSDirectory::new(avs_directory_address, provider);
    let operator_status = avs_directory
        .avsOperatorStatus(service_manager_address, operator)
        .call()
        .await?
        ._0;
    if operator_status != OperatorAVSRegistrationStatus::REGISTERED().into() {
        return Err(
            anyhow!("Operator is not registered {operator} in {service_manager_address}").into(),
        );
    }

    // Check operator signature matches
    let stake_registry = service_manager_contract.stakeRegistry().call().await?._0;
    let ecdsa_signature = ECDSAStakeRegistry::new(stake_registry, provider);
    let expected_address = ecdsa_signature
        .getOperatorSigningKeyAtBlock(operator, U256::from(signed_payload.signed_block_height))
        .call()
        .await?
        ._0;

    let signed_payload_signature: PrimitiveSignature =
        signed_payload.signature.as_slice().try_into()?;

    let signature_address =
        signed_payload_signature.recover_address_from_prehash(&signed_payload.data_hash)?;

    if expected_address != signature_address {
        return Err(anyhow!(
            "Operator signature does not match (expected address {}, got {})",
            expected_address,
            signature_address
        )
        .into());
    }

    Ok(())
}
