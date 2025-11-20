use std::{collections::HashSet, time::Duration};

use wasmtime::Trap;
use wavs_types::{TriggerAction, WasmResponse};

use crate::{utils::error::EngineError, worlds::instance::InstanceDeps};

pub async fn execute(
    deps: &mut InstanceDeps,
    trigger: TriggerAction,
) -> Result<Vec<WasmResponse>, EngineError> {
    let service_id = trigger.config.service_id.clone();
    let workflow_id = trigger.config.workflow_id.clone();
    let input: crate::bindings::operator::world::wavs::operator::input::TriggerAction =
        trigger.try_into().map_err(EngineError::Input)?;

    // Even though we have epochs forcing timeouts within WASI
    // we still need to set a timeout on the host side since we need to cancel sleeping components too
    // see https://github.com/bytecodealliance/wasmtime-go/issues/233#issuecomment-2356238658
    let responses: Vec<WasmResponse> =
        tokio::time::timeout(Duration::from_secs(deps.time_limit_seconds), {
            let service_id = service_id.clone();
            let workflow_id = workflow_id.clone();
            async move {
                crate::bindings::operator::world::WavsWorld::instantiate_async(
                    deps.store.as_operator_mut(),
                    &deps.component,
                    deps.linker.as_operator_ref(),
                )
                .await
                .map_err(EngineError::Instantiate)?
                .call_run(deps.store.as_operator_mut(), &input)
                .await
                .map_err(|e| match e.downcast_ref::<Trap>() {
                    Some(t) if *t == Trap::OutOfFuel => {
                        EngineError::OutOfFuel(service_id, workflow_id)
                    }
                    Some(t) if *t == Trap::Interrupt => {
                        EngineError::OutOfTime(service_id, workflow_id)
                    }
                    _ => EngineError::ComponentError(e),
                })?
                .map_err(EngineError::ExecResult)
                .map(|r| r.into_iter().map(|r| r.into()).collect())
            }
        })
        .await
        .map_err(|_| EngineError::OutOfTime(service_id.clone(), workflow_id.clone()))??;

    // Invariant: If there are multiple responses, they must all have an event id salt
    if responses.len() > 1 {
        let mut seen_salt = HashSet::new();
        for response in &responses {
            match &response.event_id_salt {
                Some(salt) => {
                    if !seen_salt.insert(salt) {
                        tracing::warn!(
                            service.id = %service_id,
                            workflow.id = %workflow_id,
                            "Duplicate event-id-salt: {}", const_hex::encode(salt)
                        );
                    }
                }
                None => {
                    return Err(EngineError::MissingEventIdSalt);
                }
            }
        }
    }

    Ok(responses)
}
