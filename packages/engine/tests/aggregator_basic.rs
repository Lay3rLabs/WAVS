mod helpers;

use std::collections::BTreeMap;

use crate::helpers::{aggregator_exec::execute_aggregator_component, service::make_service};
use alloy_primitives::Address;
use utils::init_tracing_tests;
use wavs_engine::bindings::aggregator::world::wavs::aggregator::aggregator::AggregatorAction;
use wavs_types::{ComponentDigest, Envelope, EnvelopeSignature, Packet, SignatureKind};

const COMPONENT_SIMPLE_AGGREGATOR_BYTES: &[u8] =
    include_bytes!("../../../examples/build/components/simple_aggregator.wasm");

#[tokio::test]
async fn basic_aggregator_execution() {
    init_tracing_tests();

    let expected_chain = "evm:31337";
    let expected_address = Address::ZERO;
    let service = make_service(
        ComponentDigest::hash(COMPONENT_SIMPLE_AGGREGATOR_BYTES),
        BTreeMap::from([
            ("chain".to_string(), expected_chain.to_string()),
            ("service_handler".to_string(), expected_address.to_string()),
        ]),
    );
    let workflow_id = service.workflows.keys().last().unwrap().clone();

    let packet = Packet {
        service,
        workflow_id,
        envelope: Envelope {
            eventId: [1u8; 20].into(),
            ordering: [0u8; 12].into(),
            payload: vec![].into(),
        },
        signature: EnvelopeSignature {
            data: alloy_primitives::Signature::from_bytes_and_parity(&[0u8; 64], false).into(),
            kind: SignatureKind::evm_default(),
        },
        trigger_data: wavs_types::TriggerData::default(),
    };

    let actions = execute_aggregator_component(COMPONENT_SIMPLE_AGGREGATOR_BYTES, packet).await;

    assert_eq!(actions.len(), 1, "Expected one action");

    match &actions[0] {
        // currently hardcoded in the aggregator component
        AggregatorAction::Submit(submit_action) => {
            assert_eq!(submit_action.chain, expected_chain);
            assert_eq!(
                submit_action.contract_address.raw_bytes,
                expected_address.into_array()
            );
        }
        _ => panic!("Expected Submit action, got {:?}", &actions[0]),
    }
}
