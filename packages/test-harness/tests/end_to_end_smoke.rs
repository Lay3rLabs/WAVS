//! End-to-end lifecycle smoke tests — the path called out by the PR review:
//! `trigger -> operator (real WASM) -> aggregator (real WASM) -> sign envelope ->
//! submit on-chain -> assert`.
//!
//! Two flavors ship here:
//!
//! - `lifecycle_component_through_aggregator_*` runs the FULL path including
//!   the aggregator stage. This is the test that satisfies issue #1147's
//!   acceptance criterion for the realistic end-to-end path.
//! - `lifecycle_component_then_sign_then_submit_without_aggregator` skips the
//!   aggregator hop. Kept because the supporting negative-case tests
//!   (`validate_rejects_*`) test the *handler's* `validate()` directly — those
//!   gain no coverage from the extra aggregator hop and stay simple.
//!
//! Skips gracefully if `examples/build/components/{echo_data,simple_aggregator}.wasm`
//! is missing.

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
        InProcRunner, MockHandler, MockHandlerConfig, RunnerAggregatorAction, RunnerSubmitAction,
        ServiceSpec,
    },
};
use wavs_types::AggregatorInput;

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
async fn lifecycle_component_then_sign_then_submit_without_aggregator() {
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
    let sigdata = sign_envelope(&env_msg, std::slice::from_ref(&operator), reference_block)
        .expect("sign envelope");

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
    // sign_envelope sorts automatically for harness safety; manually break the
    // ordering here to keep coverage for the handler's InvalidSignatureOrder path.
    let mut sigdata = sign_envelope(&env_msg, &[high, low], reference_block).unwrap();
    sigdata.signers.reverse();
    sigdata.signatures.reverse();

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

    // Build sigdata from callers in the wrong order; sign_envelope sorts it
    // automatically before returning.
    let sigdata = sign_envelope(&env_msg, &[high, low], reference_block).unwrap();

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

/// The realistic end-to-end path called out by #1147:
///
/// `trigger -> operator (real WASM) -> aggregator (real WASM) -> sign envelope ->
///  on-chain handleSignedEnvelope -> handler state assertion`.
///
/// What's exercised:
/// 1. `InProcRunner::run_component_full` runs the operator WASM and returns the
///    full `WasmResponse` (payload + ordering + event_id_salt).
/// 2. `InProcRunner::run_aggregator` runs the **aggregator WASM** on that
///    response and returns the routing `SubmitAction` — production-shape.
/// 3. The aggregator's `SubmitAction` is asserted to target the deployed
///    handler address + the registered chain key. This is the proof the
///    aggregator stage actually ran.
/// 4. `Envelope::from_operator_response` builds the on-chain envelope using
///    the aggregator-derived `EventId` (so the production-side EventId
///    derivation rule is exercised end-to-end).
/// 5. The harness signs the envelope and submits via the handler's
///    `handleSignedEnvelope`; the test asserts the handler stored the trigger
///    payload + signer set.
///
/// Caveat: `simple_aggregator.wasm` is pass-through (no quorum check). A
/// multi-operator quorum smoke test belongs in a follow-up with a
/// quorum-aware aggregator component. This test exercises the **lifecycle
/// stage**, not quorum semantics.
#[tokio::test]
async fn lifecycle_component_through_aggregator_then_sign_then_submit_then_assert() {
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

    // 1. Anvil + wallet-bound provider.
    let (provider, anvil, _deployer) = chain::spawn_local_with_deployer()
        .await
        .expect("spawn local anvil");
    let anvil_endpoint = anvil.endpoint();

    // 2. Deploy SimpleServiceManager + SimpleSubmit; register one operator.
    let operator = PrivateKeySigner::random();
    let handler_config = MockHandlerConfig::single_operator(operator.address());
    let handler = MockHandler::deploy(provider.clone(), &handler_config)
        .await
        .expect("deploy MockHandler");

    // 3. Build the spec WITH chain configs + the aggregator's config vars.
    //    The aggregator reads `chain` and `service_handler` from
    //    host::config_var; without an `evm:local` entry in chain_configs,
    //    `host::get_evm_chain_config(...)` would return None and the
    //    aggregator would error out.
    let chain_key_str = "evm:local";
    let spec = ServiceSpec::new()
        .component_wasm(&component)
        .aggregator_wasm(&aggregator)
        .with_evm_local_chain("local", &anvil_endpoint)
        .config_var("chain", chain_key_str)
        .config_var("service_handler", handler.handler.to_string())
        .operator_count(1);
    let runner = InProcRunner::from_spec(&spec).expect("build runner");

    // 4. Operator stage — drive echo_data with an ABI-encoded DataWithId.
    let trigger_id_value: u64 = 71;
    let payload_in = data_with_id_payload(trigger_id_value, b"e2e-through-aggregator");
    let trigger_action = runner.default_trigger_action(payload_in.clone());

    let responses = runner
        .run_component_full(trigger_action.clone())
        .await
        .expect("run component");
    assert_eq!(responses.len(), 1, "echo_data emits exactly one response");
    let operator_response = responses.into_iter().next().unwrap();
    assert_eq!(
        operator_response.payload, payload_in,
        "echo_data should return its input verbatim"
    );

    // 5. AGGREGATOR STAGE — the path the reviewer said was missing.
    let agg_input = AggregatorInput {
        trigger_action: trigger_action.clone(),
        operator_response: operator_response.clone(),
    };
    let event_id = agg_input
        .event_id()
        .expect("derive EventId for aggregation");
    eprintln!("[harness] derived EventId: {:?}", event_id.as_bytes());

    let actions = runner
        .run_aggregator(event_id.clone(), agg_input)
        .await
        .expect("run aggregator");
    assert_eq!(
        actions.len(),
        1,
        "simple_aggregator emits exactly one Submit per packet"
    );

    // Assert the aggregator emitted a Submit targeting our handler.
    let RunnerAggregatorAction::Submit(submit_action) = &actions[0] else {
        panic!("expected Submit action, got {:?}", &actions[0]);
    };
    match submit_action {
        RunnerSubmitAction::Evm(evm) => {
            // chain string matches what we configured.
            assert_eq!(evm.chain, chain_key_str, "aggregator chain mismatch");
            // address bytes match the deployed handler.
            assert_eq!(
                &evm.address.raw_bytes[..],
                handler.handler.as_slice(),
                "aggregator must submit to the deployed handler address"
            );
        }
        RunnerSubmitAction::Cosmos(_) => panic!("expected Evm submit action"),
    }
    eprintln!("[harness] aggregator Submit action verified — chain + handler match");

    // 6. Build the envelope from the operator response using the
    //    aggregator-derived EventId (mirrors production submission.rs).
    let env_msg = Envelope::from_operator_response(event_id, &operator_response);

    let block_number = provider.get_block_number().await.expect("block number");
    let reference_block = block_number.saturating_sub(1) as u32;
    let sigdata = sign_envelope(&env_msg, std::slice::from_ref(&operator), reference_block)
        .expect("sign envelope");

    // 7. Submit + assert.
    let receipt = handler
        .submit_envelope(&env_msg, &sigdata)
        .await
        .expect("submit envelope");
    assert!(receipt.status(), "handleSignedEnvelope must succeed");

    assert!(
        handler
            .is_valid_trigger(trigger_id_value)
            .await
            .expect("isValidTriggerId"),
        "handler must mark triggerId {trigger_id_value} valid after handleSignedEnvelope"
    );

    let stored = SimpleSubmit::new(handler.handler, &provider)
        .getSignedData(trigger_id_value)
        .call()
        .await
        .expect("getSignedData");
    assert_eq!(
        stored.envelope.payload, env_msg.payload,
        "stored payload must match what the aggregator routed + we submitted"
    );
    assert_eq!(stored.signatureData.signers[0], operator.address());

    // Suppress unused warnings on builds where U256 isn't otherwise touched.
    let _ = U256::ZERO;
}
