use wasmtime::Trap;
use wavs_types::TriggerAction;

use crate::{EngineError, InstanceDeps};

pub async fn execute(
    deps: &mut InstanceDeps,
    trigger: TriggerAction,
) -> Result<Vec<u8>, EngineError> {
    let service_id = trigger.config.service_id.clone();
    let workflow_id = trigger.config.workflow_id.clone();
    let input: crate::bindings::world::wavs::worker::layer_types::TriggerAction =
        trigger.try_into()?;

    crate::bindings::world::LayerTriggerWorld::instantiate_async(
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
        _ => EngineError::ComponentError(e),
    })?
    .map_err(EngineError::ExecResult)
}
