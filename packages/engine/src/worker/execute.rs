use std::time::Duration;

use wasmtime::Trap;
use wavs_types::{TriggerAction, WasmResponse};

use super::instance::InstanceDeps;
use crate::EngineError;

pub async fn execute(
    deps: &mut InstanceDeps,
    trigger: TriggerAction,
) -> Result<Option<WasmResponse>, EngineError> {
    let service_id = trigger.config.service_id.clone();
    let workflow_id = trigger.config.workflow_id.clone();
    let input: super::bindings::world::wavs::worker::input::TriggerAction =
        trigger.try_into().map_err(EngineError::Input)?;

    // Even though we have epochs forcing timeouts within WASI
    // we still need to set a timeout on the host side
    // see https://github.com/bytecodealliance/wasmtime-go/issues/233#issuecomment-2356238658
    tokio::time::timeout(Duration::from_secs(deps.time_limit_seconds), {
        let service_id = service_id.clone();
        let workflow_id = workflow_id.clone();
        async move {
            super::bindings::world::WavsWorld::instantiate_async(
                &mut deps.store,
                &deps.component,
                &deps.linker,
            )
            .await
            .map_err(EngineError::Instantiate)?
            .call_run(&mut deps.store, &input)
            .await
            .map_err(|e| match e.downcast_ref::<Trap>() {
                Some(t) if *t == Trap::OutOfFuel => EngineError::OutOfFuel(service_id, workflow_id),
                Some(t) if *t == Trap::Interrupt => EngineError::OutOfTime(service_id, workflow_id),
                _ => EngineError::ComponentError(e),
            })?
            .map_err(EngineError::ExecResult)
            .map(|res| res.map(|r| r.into()))
        }
    })
    .await
    .map_err(|_| EngineError::OutOfTime(service_id, workflow_id))?
}
