//! End-to-end lifecycle smoke test — the path called out by the PR review:
//!
//!   trigger → operator (real WASM) → sign envelope → submit on-chain → assert
//!
//! This test boots a local Anvil instance, deploys the `SimpleServiceManager`
//! + `SimpleSubmit` reference mocks, runs the `echo_data.wasm` operator
//! component through `InProcRunner`, signs the produced payload with an
//! operator key registered in the manager, submits the signed envelope via
//! `handleSignedEnvelope`, and asserts the handler stored the trigger as
//! valid. Then negative cases prove `validate()` actually rejects bad input.
//!
//! Skips gracefully if `examples/build/components/echo_data.wasm` is missing.

use std::path::PathBuf;

use alloy_primitives::{Bytes, U256};
use alloy_provider::Provider;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::SolValue;

use wavs_test_harness::{
    chain,
    envelope::{self, sign_envelope, Envelope},
    service::{
        handler::{ISimpleSubmit, ISimpleTrigger, SimpleSubmit},
        InProcRunner, MockHandler, MockHandlerConfig, ServiceSpec,
    },
};

fn example_wasm(name: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/build/components")
        .join(name);
    p.canonicalize().unwrap_or(p)
}

/// Build the ABI-encoded `DataWithId(triggerId, data)` payload that
/// `SimpleSubmit.handleSignedEnvelope` decodes.
fn data_with_id_payload(trigger_id: u64, data: &[u8]) -> Vec<u8> {
    let dw = ISimpleSubmit::DataWithId {
        triggerId: trigger_id,
        data: Bytes::copy_from_slice(data),
    };
    dw.abi_encode()
}

#[allow(dead_code)]
fn trigger_id(id: u64) -> ISimpleTrigger::TriggerId {
    id.into()
}

#[tokio::test]
async fn lifecycle_component_then_sign_then_submit_then_assert() {
    let _ = tracing_subscriber::fmt::try_init();

    let component = example_wasm("echo_data.wasm");
    let aggregator = example_wasm("simple_aggregator.wasm");
    if !component.exists() || !aggregator.exists() {
        eprintln!(
            "[skipping] {} or aggregator not found — run `just wasi-build-native`",
            component.display()
        );
        return;
    }

    // 1. Spawn Anvil with a wallet-bound provider so contract deploys work.
    let (provider, _anvil, _deployer) = chain::spawn_local_with_deployer()
        .await
        .expect("spawn local anvil");

    // 2. Build a signer set the manager will accept (single operator, weight 1).
    let operator = PrivateKeySigner::random();
    let config = MockHandlerConfig::single_operator(operator.address());

    let handler = MockHandler::deploy(provider.clone(), &config)
        .await
        .expect("deploy MockHandler");

    // 3. Run the operator component end-to-end through wavs-engine.
    let spec = ServiceSpec::new()
        .component_wasm(&component)
        .aggregator_wasm(&aggregator)
        .operator_count(1);
    let runner = InProcRunner::from_spec(&spec).expect("build runner");

    let trigger_id: u64 = 42;
    let payload_in = data_with_id_payload(trigger_id, b"e2e-lifecycle");
    let outputs = runner
        .run_component(payload_in.clone())
        .await
        .expect("run component");
    assert_eq!(outputs.len(), 1, "echo_data emits exactly one payload");
    // echo_data with Raw trigger data returns the input bytes verbatim.
    assert_eq!(outputs[0], payload_in, "operator output must match input");

    // 4. Build + sign envelope. SimpleServiceManager requires referenceBlock <
    //    block.number; we just-mined block 0 on Anvil, so use 0.
    let event_id = envelope::event_id_from_nonce(trigger_id);
    let env_msg = Envelope::new(event_id, outputs[0].clone());
    let block_number = provider.get_block_number().await.expect("block number");
    let reference_block = if block_number > 0 {
        block_number - 1
    } else {
        0
    } as u32;
    let sigdata =
        sign_envelope(&env_msg, &[operator.clone()], reference_block).expect("sign envelope");

    // 5. Submit on-chain and watch the receipt.
    let receipt = handler
        .submit_envelope(&env_msg, &sigdata)
        .await
        .expect("submit envelope");
    assert!(receipt.status(), "handleSignedEnvelope must succeed");

    // 6. Assert handler state changed.
    let valid = handler
        .is_valid_trigger(trigger_id)
        .await
        .expect("isValidTriggerId");
    assert!(
        valid,
        "handler must mark triggerId {trigger_id} valid after handleSignedEnvelope"
    );

    // 7. Verify the stored signed-data matches what we submitted.
    let stored = SimpleSubmit::new(handler.handler, &provider)
        .getSignedData(trigger_id)
        .call()
        .await
        .expect("getSignedData");
    assert_eq!(
        stored.envelope.payload, env_msg.payload,
        "stored envelope payload must equal what we submitted"
    );
    assert_eq!(
        stored.signatureData.signers.len(),
        1,
        "stored sigdata must have one signer"
    );
    assert_eq!(stored.signatureData.signers[0], operator.address());
}

#[tokio::test]
async fn validate_rejects_zero_weight_signer() {
    let _ = tracing_subscriber::fmt::try_init();

    let component = example_wasm("echo_data.wasm");
    if !component.exists() {
        eprintln!("[skipping] echo_data.wasm missing");
        return;
    }

    let (provider, _anvil, _deployer) = chain::spawn_local_with_deployer()
        .await
        .expect("spawn local anvil");

    // Build the manager with operator A registered, but sign with operator B.
    let registered = PrivateKeySigner::random();
    let mut config = MockHandlerConfig::single_operator(registered.address());
    config.threshold_weight = U256::from(1);
    let handler = MockHandler::deploy(provider.clone(), &config)
        .await
        .expect("deploy handler");

    let unregistered = PrivateKeySigner::random();
    let payload = data_with_id_payload(7, b"no-quorum");
    let env_msg = Envelope::new(envelope::event_id_from_nonce(7), payload);

    let block_number = provider.get_block_number().await.unwrap();
    let reference_block = if block_number > 0 {
        block_number - 1
    } else {
        0
    } as u32;
    let sigdata = sign_envelope(&env_msg, &[unregistered], reference_block).unwrap();

    let res = handler.submit_envelope(&env_msg, &sigdata).await;
    assert!(
        res.is_err(),
        "submission with unregistered signer must revert (InsufficientQuorumZero / Insufficient Quorum)"
    );
}

#[tokio::test]
async fn validate_rejects_out_of_order_signers() {
    let _ = tracing_subscriber::fmt::try_init();

    let component = example_wasm("echo_data.wasm");
    if !component.exists() {
        eprintln!("[skipping] echo_data.wasm missing");
        return;
    }

    let (provider, _anvil, _deployer) = chain::spawn_local_with_deployer()
        .await
        .expect("spawn local anvil");

    // Two operators, quorum 2.
    let s1 = PrivateKeySigner::random();
    let s2 = PrivateKeySigner::random();
    // SimpleServiceManager requires strict ascending order. Pick the lexically
    // larger first to force an out-of-order array.
    let (high, low) = if s1.address() > s2.address() {
        (s1.clone(), s2.clone())
    } else {
        (s2.clone(), s1.clone())
    };

    let config = MockHandlerConfig::quorum_of(&[s1.address(), s2.address()], 2).unwrap();
    let handler = MockHandler::deploy(provider.clone(), &config)
        .await
        .expect("deploy handler");

    let payload = data_with_id_payload(9, b"out-of-order");
    let env_msg = Envelope::new(envelope::event_id_from_nonce(9), payload);
    let block_number = provider.get_block_number().await.unwrap();
    let reference_block = if block_number > 0 {
        block_number - 1
    } else {
        0
    } as u32;
    // Submit with `high` first to trigger InvalidSignatureOrder.
    let sigdata = sign_envelope(&env_msg, &[high, low], reference_block).unwrap();

    let res = handler.submit_envelope(&env_msg, &sigdata).await;
    assert!(
        res.is_err(),
        "unsorted signer array must revert with InvalidSignatureOrder"
    );
}

#[tokio::test]
async fn sort_signature_data_lets_submission_succeed() {
    let _ = tracing_subscriber::fmt::try_init();

    let component = example_wasm("echo_data.wasm");
    if !component.exists() {
        eprintln!("[skipping] echo_data.wasm missing");
        return;
    }

    let (provider, _anvil, _deployer) = chain::spawn_local_with_deployer()
        .await
        .expect("spawn local anvil");

    let s1 = PrivateKeySigner::random();
    let s2 = PrivateKeySigner::random();
    let (high, low) = if s1.address() > s2.address() {
        (s1.clone(), s2.clone())
    } else {
        (s2.clone(), s1.clone())
    };
    let config = MockHandlerConfig::quorum_of(&[s1.address(), s2.address()], 2).unwrap();
    let handler = MockHandler::deploy(provider.clone(), &config)
        .await
        .expect("deploy handler");

    let payload = data_with_id_payload(11, b"sort-and-submit");
    let env_msg = Envelope::new(envelope::event_id_from_nonce(11), payload);
    let block_number = provider.get_block_number().await.unwrap();
    let reference_block = if block_number > 0 {
        block_number - 1
    } else {
        0
    } as u32;

    // Build sigdata in the wrong order on purpose, then sort.
    let mut sigdata = sign_envelope(&env_msg, &[high, low], reference_block).unwrap();
    envelope::sort_signature_data(&mut sigdata);

    let receipt = handler
        .submit_envelope(&env_msg, &sigdata)
        .await
        .expect("submit after sort");
    assert!(
        receipt.status(),
        "sorted multi-signer submission must succeed"
    );
    assert!(handler.is_valid_trigger(11).await.unwrap());
}
