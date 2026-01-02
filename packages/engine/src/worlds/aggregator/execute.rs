use std::time::Duration;

use anyhow::Result;
use wasmtime::Trap;
use wavs_types::AggregatorInput;

use crate::utils::error::EngineError;
use crate::{bindings::aggregator::world::AggregatorWorld, worlds::instance::InstanceDeps};

pub use crate::bindings::aggregator::world::wavs::aggregator::output::{
    AggregatorAction, SubmitAction,
};
use crate::bindings::aggregator::world::wavs::types::chain::AnyTxHash;

pub async fn execute_input(
    deps: &mut InstanceDeps,
    input: AggregatorInput,
) -> Result<Vec<AggregatorAction>, EngineError> {
    let service_id = input.trigger_action.config.service_id.clone();
    let workflow_id = input.trigger_action.config.workflow_id.clone();

    let wit_input = input.try_into().map_err(EngineError::Input)?;

    tokio::time::timeout(Duration::from_secs(deps.time_limit_seconds), {
        let service_id = service_id.clone();
        let workflow_id = workflow_id.clone();
        async move {
            AggregatorWorld::instantiate_async(
                deps.store.as_aggregator_mut(),
                &deps.component,
                deps.linker.as_aggregator_ref(),
            )
            .await
            .map_err(EngineError::Instantiate)?
            .call_process_input(deps.store.as_aggregator_mut(), &wit_input)
            .await
            .map_err(|e| match e.downcast_ref::<Trap>() {
                Some(t) if *t == Trap::OutOfFuel => EngineError::OutOfFuel(service_id, workflow_id),
                Some(t) if *t == Trap::Interrupt => EngineError::OutOfTime(service_id, workflow_id),
                _ => EngineError::ComponentError(e),
            })?
            .map_err(|error| {
                EngineError::ExecResult(format!("Process packet execution failed: {}", error))
            })
        }
    })
    .await
    .map_err(|_| EngineError::OutOfTime(service_id, workflow_id))?
}

pub async fn execute_timer_callback(
    deps: &mut InstanceDeps,
    input: AggregatorInput,
) -> Result<Vec<AggregatorAction>, EngineError> {
    let service_id = input.trigger_action.config.service_id.clone();
    let workflow_id = input.trigger_action.config.workflow_id.clone();
    let wit_input = input.try_into().map_err(EngineError::Input)?;

    tokio::time::timeout(Duration::from_secs(deps.time_limit_seconds), {
        let service_id = service_id.clone();
        let workflow_id = workflow_id.clone();
        async move {
            AggregatorWorld::instantiate_async(
                deps.store.as_aggregator_mut(),
                &deps.component,
                deps.linker.as_aggregator_ref(),
            )
            .await
            .map_err(EngineError::Instantiate)?
            .call_handle_timer_callback(deps.store.as_aggregator_mut(), &wit_input)
            .await
            .map_err(|e| match e.downcast_ref::<Trap>() {
                Some(t) if *t == Trap::OutOfFuel => EngineError::OutOfFuel(service_id, workflow_id),
                Some(t) if *t == Trap::Interrupt => EngineError::OutOfTime(service_id, workflow_id),
                _ => EngineError::ComponentError(e),
            })?
            .map_err(|error| {
                EngineError::ExecResult(format!("Timer callback execution failed: {}", error))
            })
        }
    })
    .await
    .map_err(|_| EngineError::OutOfTime(service_id, workflow_id))?
}

pub async fn execute_submit_callback(
    deps: &mut InstanceDeps,
    input: AggregatorInput,
    tx_result: Result<AnyTxHash, String>,
) -> Result<(), EngineError> {
    let service_id = input.trigger_action.config.service_id.clone();
    let workflow_id = input.trigger_action.config.workflow_id.clone();
    let wit_input = input.try_into().map_err(EngineError::Input)?;
    let wit_tx_result = tx_result.as_ref().map_err(|e| e.as_str());

    tokio::time::timeout(Duration::from_secs(deps.time_limit_seconds), {
        let service_id = service_id.clone();
        let workflow_id = workflow_id.clone();
        async move {
            AggregatorWorld::instantiate_async(
                deps.store.as_aggregator_mut(),
                &deps.component,
                deps.linker.as_aggregator_ref(),
            )
            .await
            .map_err(EngineError::Instantiate)?
            .call_handle_submit_callback(deps.store.as_aggregator_mut(), &wit_input, wit_tx_result)
            .await
            .map_err(|e| match e.downcast_ref::<Trap>() {
                Some(t) if *t == Trap::OutOfFuel => EngineError::OutOfFuel(service_id, workflow_id),
                Some(t) if *t == Trap::Interrupt => EngineError::OutOfTime(service_id, workflow_id),
                _ => EngineError::ComponentError(e),
            })?
            .map_err(|error| {
                EngineError::ExecResult(format!("Submit callback execution failed: {}", error))
            })
        }
    })
    .await
    .map_err(|_| EngineError::OutOfTime(service_id, workflow_id))?
}
