mod helpers;

use crate::helpers::aggregator_exec::execute_aggregator_component;
use utils::init_tracing_tests;
use wavs_engine::bindings::aggregator::world::wavs::aggregator::aggregator::AggregatorAction;
use wavs_types::{
    Envelope, EnvelopeSignature, Packet, Service, ServiceManager, ServiceStatus, SignatureKind,
    WorkflowId,
};

const COMPONENT_SIMPLE_AGGREGATOR_BYTES: &[u8] =
    include_bytes!("../../../examples/build/components/simple_aggregator.wasm");

#[tokio::test]
async fn basic_aggregator_execution() {
    init_tracing_tests();

    let packet = Packet {
        service: Service {
            name: "test-service".to_string(),
            workflows: Default::default(),
            status: ServiceStatus::Active,
            manager: ServiceManager::Evm {
                chain: "evm:31337".try_into().unwrap(),
                address: [0u8; 20].into(),
            },
        },
        workflow_id: WorkflowId::new("test-workflow").unwrap(),
        envelope: Envelope {
            eventId: [0u8; 20].into(),
            ordering: [0u8; 12].into(),
            payload: vec![].into(),
        },
        signature: EnvelopeSignature {
            data: alloy_primitives::Signature::from_bytes_and_parity(&[0u8; 64], false).into(),
            kind: SignatureKind::evm_default(),
        },
        origin_tx_hash: vec![],  // Empty for test
        origin_block: 0, // Dummy block number
    };

    let actions = execute_aggregator_component(COMPONENT_SIMPLE_AGGREGATOR_BYTES, packet).await;

    assert_eq!(actions.len(), 1, "Expected one action");

    match &actions[0] {
        // currently hardcoded in the aggregator component
        AggregatorAction::Submit(submit_action) => {
            assert_eq!(submit_action.chain, "evm:31337");
            assert_eq!(submit_action.contract_address.raw_bytes, vec![0u8; 20]);
        }
        _ => panic!("Expected Submit action, got {:?}", &actions[0]),
    }
}
