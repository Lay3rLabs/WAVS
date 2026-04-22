mod helpers;

use std::collections::BTreeMap;

use crate::helpers::{aggregator_exec::execute_aggregator_component, service::make_service};
use alloy_primitives::Address;
use utils::init_tracing_tests;
use wavs_engine::bindings::aggregator::world::wavs::aggregator::output::AggregatorAction;
use wavs_types::{AggregatorInput, ComponentDigest, EvmSubmitAction, SubmitAction, WasmResponse};

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

    let input = AggregatorInput {
        trigger_action: wavs_types::TriggerAction {
            config: wavs_types::TriggerConfig {
                service_id: service.id(),
                workflow_id,
                trigger: service.workflows.iter().next().unwrap().1.trigger.clone(),
            },
            data: wavs_types::TriggerData::default(),
        },
        operator_response: WasmResponse {
            event_id_salt: None,
            payload: vec![],
            ordering: None,
        },
    };

    let actions =
        execute_aggregator_component(COMPONENT_SIMPLE_AGGREGATOR_BYTES, input, service).await;

    assert_eq!(actions.len(), 1, "Expected one action");

    match &actions[0] {
        // currently hardcoded in the aggregator component
        AggregatorAction::Submit(submit_action) => {
            match wavs_types::SubmitAction::try_from(submit_action.clone()).unwrap() {
                SubmitAction::Evm(EvmSubmitAction { chain, address, .. }) => {
                    assert_eq!(chain, expected_chain.parse().unwrap());
                    assert_eq!(address, expected_address.into());
                }
                _ => panic!("Expected Evm Submit action, got {:?}", submit_action),
            }
        }
        _ => panic!("Expected Submit action, got {:?}", &actions[0]),
    }
}
