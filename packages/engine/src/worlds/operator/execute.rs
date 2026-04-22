use std::{collections::HashSet, time::Duration};

use wasmtime::Trap;
use wasmtime::component::types::ComponentItem;
use wavs_types::{ServiceId, TriggerAction, WasmResponse, Workflow, WorkflowId};

use crate::{utils::error::EngineError, worlds::instance::InstanceDeps};

/// Check if a compiled component exports the `agent` named interface.
/// Used to determine whether to use the continuation loop (call_run_agent)
/// or the legacy single-shot path (call_run).
fn has_agent_export(component: &wasmtime::component::Component, engine: &wasmtime::Engine) -> bool {
    let component_type = component.component_type();
    for (name, item) in component_type.exports(engine) {
        // Named interface export from `export agent;` in wavs-world
        // appears as ComponentItem::ComponentInstance with name containing "agent"
        if matches!(item, ComponentItem::ComponentInstance(_)) && name.contains("agent") {
            return true;
        }
    }
    false
}

pub async fn execute(
    deps: &mut InstanceDeps,
    trigger: TriggerAction,
    max_payload_size: usize,
    max_salt_size: usize,
) -> Result<Vec<WasmResponse>, EngineError> {
    let service_id = trigger.config.service_id.clone();
    let workflow_id = trigger.config.workflow_id.clone();
    let input: crate::bindings::operator::world::wavs::operator::input::TriggerAction =
        trigger.try_into().map_err(EngineError::Input)?;

    // Get the wasmtime Engine from the store to inspect component exports
    let engine = deps.store.as_operator_mut().engine().clone();
    let is_agent = has_agent_export(&deps.component, &engine);

    let responses: Vec<WasmResponse> = if is_agent {
        execute_agent(deps, &input, &service_id, &workflow_id).await?
    } else {
        execute_legacy(deps, &input, &service_id, &workflow_id).await?
    };

    // Validate response sizes
    for response in &responses {
        response.validate_size(max_payload_size, max_salt_size)?;
    }

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

/// Legacy single-shot execution path for non-agent components.
async fn execute_legacy(
    deps: &mut InstanceDeps,
    input: &crate::bindings::operator::world::wavs::operator::input::TriggerAction,
    service_id: &ServiceId,
    workflow_id: &WorkflowId,
) -> Result<Vec<WasmResponse>, EngineError> {
    // Even though we have epochs forcing timeouts within WASI
    // we still need to set a timeout on the host side since we need to cancel sleeping components too
    // see https://github.com/bytecodealliance/wasmtime-go/issues/233#issuecomment-2356238658
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
            .map_err(|e| EngineError::Instantiate(e.into()))?
            .call_run(deps.store.as_operator_mut(), input)
            .await
            .map_err(|e| match e.downcast_ref::<Trap>() {
                Some(t) if *t == Trap::OutOfFuel => {
                    EngineError::OutOfFuel(service_id, workflow_id)
                }
                Some(t) if *t == Trap::Interrupt => {
                    EngineError::OutOfTime(service_id, workflow_id)
                }
                _ => EngineError::ComponentError(e.into()),
            })?
            .map_err(EngineError::ExecResult)
            .map(|r| r.into_iter().map(|r| r.into()).collect())
        }
    })
    .await
    .map_err(|_| EngineError::OutOfTime(service_id.clone(), workflow_id.clone()))?
}

/// Agent continuation loop — re-invokes the agent until it returns Done or the step limit is hit.
///
/// The loop:
/// 1. Calls `call_run_agent` on the component
/// 2. On `Continue(step_name)`: persists step_name to KV store, increments step counter, resets fuel
/// 3. On `Done(responses)`: returns the responses
/// 4. On step limit exceeded: returns `ContinuationLimit` error
async fn execute_agent(
    deps: &mut InstanceDeps,
    input: &crate::bindings::operator::world::wavs::operator::input::TriggerAction,
    service_id: &ServiceId,
    workflow_id: &WorkflowId,
) -> Result<Vec<WasmResponse>, EngineError> {
    use crate::bindings::operator::world::wavs::operator::output::StepResult;

    // Extract max_continuation_steps and DB context BEFORE the loop to avoid borrow conflicts
    let (max_steps, db, kv_namespace, fuel_limit) = {
        let store = deps.store.as_operator_mut();
        let host = store.data();

        let max_steps = host
            .service
            .workflows
            .get(workflow_id)
            .and_then(|w| w.component.max_continuation_steps)
            .unwrap_or(10) as usize;

        let fuel_limit = host
            .service
            .workflows
            .get(workflow_id)
            .and_then(|w| w.component.fuel_limit)
            .unwrap_or(Workflow::DEFAULT_FUEL_LIMIT);

        // Clone WavsDb — cheap because it wraps a DashMap (Arc internally)
        let db = host.keyvalue_ctx.db();
        let kv_namespace = host.service.id().to_string();

        (max_steps, db, kv_namespace, fuel_limit)
    };

    // LRU pin: hold an Arc clone of the compiled component for the loop's lifetime.
    // This prevents the LRU cache from evicting the compiled module even under memory pressure.
    let _component_pin = deps.component.clone();

    // Correlation ID: unique per (service, workflow) invocation — used as KV key component
    let correlation_id = format!("{}:{}", service_id, workflow_id);

    let mut step: usize = 0;

    loop {
        if step >= max_steps {
            return Err(EngineError::ContinuationLimit {
                service_id: service_id.clone(),
                workflow_id: workflow_id.clone(),
                steps: max_steps,
            });
        }

        tracing::info!(
            service_id = %service_id,
            workflow_id = %workflow_id,
            step = step,
            max_steps = max_steps,
            "Agent continuation step"
        );

        // Per-step timeout: each step gets the full time_limit_seconds budget
        let step_result = tokio::time::timeout(
            Duration::from_secs(deps.time_limit_seconds),
            async {
                let world = crate::bindings::operator::world::WavsWorld::instantiate_async(
                    deps.store.as_operator_mut(),
                    &deps.component,
                    deps.linker.as_operator_ref(),
                )
                .await
                .map_err(|e| EngineError::Instantiate(e.into()))?;

                world
                    .wavs_operator_agent()
                    .call_run_agent(deps.store.as_operator_mut(), input)
                    .await
                    .map_err(|e| match e.downcast_ref::<Trap>() {
                        Some(t) if *t == Trap::Interrupt => {
                            EngineError::OutOfTime(service_id.clone(), workflow_id.clone())
                        }
                        Some(t) if *t == Trap::OutOfFuel => {
                            EngineError::OutOfFuel(service_id.clone(), workflow_id.clone())
                        }
                        _ => EngineError::ComponentError(e.into()),
                    })?
                    .map_err(EngineError::ExecResult)
            },
        )
        .await
        .map_err(|_| EngineError::OutOfTime(service_id.clone(), workflow_id.clone()))??;

        match step_result {
            StepResult::Done(responses) => {
                tracing::info!(
                    service_id = %service_id,
                    workflow_id = %workflow_id,
                    total_steps = step + 1,
                    "Agent continuation completed with Done"
                );
                return Ok(responses.into_iter().map(|r| r.into()).collect());
            }
            StepResult::Continue(step_name) => {
                // Persist the step name to KV so the component can read it on next invocation.
                // Key format: {namespace}/wavs_agent_step/{correlation_id}:step:{N}
                // Component reads via: bucket.open("wavs_agent_step").get("{correlation_id}:step:{N}")
                // since the KV layer prepends "{namespace}/wavs_agent_step/" automatically.
                let kv_key = format!(
                    "{}/wavs_agent_step/{}:step:{}",
                    kv_namespace, correlation_id, step
                );
                if let Err(e) = db.kv_store.insert(kv_key.clone(), step_name.as_bytes().to_vec()) {
                    tracing::warn!(
                        service_id = %service_id,
                        key = %kv_key,
                        error = %e,
                        "Failed to persist continuation state to KV"
                    );
                }

                tracing::debug!(
                    service_id = %service_id,
                    step = step,
                    step_name = %step_name,
                    kv_key = %kv_key,
                    "Agent continuing to next step"
                );

                step += 1;

                // Reset fuel for the next step so each step gets its own fuel budget.
                // The store is reused across continuation steps; without reset the second
                // step would start with whatever fuel the first step left over.
                deps.store
                    .as_operator_mut()
                    .set_fuel(fuel_limit)
                    .map_err(|e| EngineError::Store(e.into()))?;
            }
        }
    }
}
