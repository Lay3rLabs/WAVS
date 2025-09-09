use alloy_primitives::Address;
use alloy_provider::{DynProvider, Provider};
use alloy_rpc_types_eth::TransactionReceipt;
use axum::{extract::State, response::IntoResponse, Json};
use tracing::instrument;
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    ChainKey, EnvelopeExt, EnvelopeSignature,
    IWavsServiceHandler::IWavsServiceHandlerInstance,
    IWavsServiceManager::IWavsServiceManagerInstance,
    Packet, ServiceManagerError,
};

use crate::{
    engine::{AggregatorAction, SubmitAction},
    error::{AggregatorError, AggregatorResult, PacketValidationError},
    http::{
        error::AnyError,
        state::{HttpState, QueuedPacket, QuorumQueue, QuorumQueueId},
    },
};

#[utoipa::path(
    post,
    path = "/packet",
    request_body = AddPacketRequest,
    responses(
        (status = 200, description = "Packet successfully added to queue or sent to contract", body = Vec<AddPacketResponse>),
        (status = 400, description = "Invalid packet data or signature"),
        (status = 500, description = "Internal server error during packet processing")
    ),
    description = "Validates and processes a packet, adding it to the aggregation queue. When enough packets from different signers accumulate to meet the threshold, the aggregated packet is sent to the target contract."
)]
#[axum::debug_handler]
#[instrument(level = "info", skip(state, req), fields(service.name = %req.packet.service.name, service.manager = ?req.packet.service.manager, workflow_id = %req.packet.workflow_id))]
pub async fn handle_packet(
    State(state): State<HttpState>,
    Json(req): Json<AddPacketRequest>,
) -> impl IntoResponse {
    match process_packet(state, &req.packet).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => {
            tracing::error!("{:?}", e);
            AnyError::from(e).into_response()
        }
    }
}

#[instrument(level = "debug", skip(state, packet), fields(service.name = %packet.service.name, service.manager = ?packet.service.manager, workflow_id = %packet.workflow_id))]
async fn process_packet(
    state: HttpState,
    packet: &Packet,
) -> AggregatorResult<Vec<AddPacketResponse>> {
    if !state.service_registered(packet.service.id()).await {
        return Err(AggregatorError::MissingService(packet.service.id()));
    }

    tracing::info!(
        "Processing packet for service: {}, workflow: {}",
        packet.service.id(),
        packet.workflow_id
    );

    let workflow = &packet.service.workflows[&packet.workflow_id];

    if !matches!(workflow.submit, wavs_types::Submit::Aggregator { .. }) {
        return Err(AggregatorError::MissingWorkflow {
            workflow_id: packet.workflow_id.clone(),
            service_id: packet.service.id(),
        });
    }

    // this implicitly validates that the signature is valid
    let signing_key = packet.signature.evm_signer_address(&packet.envelope)?;

    // Query for the operator address associated with this signing key
    // we can use the service manager from the staked chain for this
    // but drop it after this scope so we don't confuse it with the service manager
    // that is used for the actual submission
    let signer = {
        let service_manager_client = state.get_evm_client(packet.service.manager.chain()).await?;
        let service_manager = IWavsServiceManagerInstance::new(
            packet.service.manager.evm_address_unchecked(),
            service_manager_client.provider,
        );
        service_manager
            .getLatestOperatorForSigningKey(signing_key)
            .call()
            .await
            .map_err(AggregatorError::OperatorKeyLookup)?
    };
    tracing::debug!("Packet signer address: {:?}", signer);

    AggregatorProcess {
        state: &state,
        packet,
        signer,
    }
    .run()
    .await
}

struct AggregatorProcess<'a> {
    state: &'a HttpState,
    packet: &'a Packet,
    signer: Address,
}

impl AggregatorProcess<'_> {
    #[instrument(level = "debug", skip(self), fields(signer = ?self.signer))]
    async fn run(self) -> AggregatorResult<Vec<AddPacketResponse>> {
        let Self {
            state,
            packet,
            signer,
        } = self;

        let event_id = packet.event_id();

        let component = match &packet.service.workflows[&packet.workflow_id].submit {
            wavs_types::Submit::Aggregator { component, .. } => component,
            _ => {
                return Ok(vec![AddPacketResponse::Error {
                    reason: format!(
                        "MissingWorkflow: workflow_id: {}, service_id: {}",
                        packet.workflow_id,
                        packet.service.id()
                    ),
                }])
            }
        };

        let actions = match state
            .aggregator_engine
            .execute_packet(component, packet)
            .await
        {
            Ok(actions) => actions,
            Err(e) => {
                return Ok(vec![AddPacketResponse::Error {
                    reason: format!("ComponentExecution: {}", e),
                }])
            }
        };

        if actions.is_empty() {
            return Ok(vec![]);
        }

        let mut responses = Vec::new();

        for action in actions {
            let queue_id = QuorumQueueId {
                event_id: event_id.clone(),
                aggregator_action: action.clone().into(),
            };

            let result =
                process_action(state.clone(), packet.clone(), queue_id, action, signer).await;

            match result {
                Ok(response) => responses.push(response),
                Err(e) => responses.push(AddPacketResponse::Error {
                    reason: format!("{:?}", e),
                }),
            }
        }

        Ok(responses)
    }
}

async fn process_action(
    state: HttpState,
    packet: Packet,
    queue_id: QuorumQueueId,
    action: AggregatorAction,
    signer: Address,
) -> AggregatorResult<AddPacketResponse> {
    match &action {
        AggregatorAction::Submit(submit_action) => {
            // execute the logic within a transaction, keyed by queue_id
            // other queue ids can run concurrently, but this makes sure that
            // we lock this queue_id against changes from other requests coming in while we process it
            state
                .queue_transaction
                .run(queue_id.clone(), {
                    let state = state.clone();
                    let packet = packet.clone();
                    let submit_action = submit_action.clone();
                    move || async move {
                        let queue = match state.get_quorum_queue(&queue_id).await? {
                            QuorumQueue::Active(queue) => {
                                add_packet_to_quorum_queue(&packet, queue, signer)?
                            }
                            QuorumQueue::Burned => return Ok(AddPacketResponse::Burned),
                        };
                        match handle_custom_submit(&state, &packet, &queue, submit_action).await {
                            Ok(tx_receipt) => {
                                state
                                    .save_quorum_queue(&queue_id, QuorumQueue::Burned)
                                    .await?;
                                Ok(AddPacketResponse::Sent {
                                    tx_receipt: Box::new(tx_receipt),
                                    count: queue.len(),
                                })
                            }
                            Err(e) => {
                                if let AggregatorError::ServiceManagerValidateKnown(
                                    ServiceManagerError::InsufficientQuorum(_),
                                ) = &e
                                {
                                    let count = queue.len();
                                    state
                                        .save_quorum_queue(&queue_id, QuorumQueue::Active(queue))
                                        .await?;
                                    Ok(AddPacketResponse::Aggregated { count })
                                } else {
                                    Err(e)
                                }
                            }
                        }
                    }
                })
                .await
        }
        AggregatorAction::Timer(timer_action) => {
            let delay: wavs_types::Duration = timer_action.delay.into();
            tracing::info!("Starting timer for {} seconds", delay.secs);

            // Spawn timer callback as background task to avoid holding the async transaction lock
            tokio::spawn(handle_timer_callback(
                state.clone(),
                packet.clone(),
                queue_id.clone(),
                signer,
                delay,
            ));

            Ok(AddPacketResponse::TimerStarted {
                delay_seconds: delay.secs,
            })
        }
    }
}

async fn get_submission_service_manager(
    state: &HttpState,
    chain: &ChainKey,
    service_handler_address: Address,
) -> AggregatorResult<IWavsServiceManagerInstance<DynProvider>> {
    // we need to get the service manager from the perspective of the service handler
    // which may be different than the service manager where the operator is staked
    // e.g. in the case of operator sets that are mirrored across multiple chains
    let service_handler_client = state.get_evm_client(chain).await?;
    let service_handler = IWavsServiceHandlerInstance::new(
        service_handler_address,
        service_handler_client.provider.clone(),
    );

    let service_manager_address = service_handler
        .getServiceManager()
        .call()
        .await
        .map_err(AggregatorError::ServiceManagerLookup)?;

    Ok(IWavsServiceManagerInstance::new(
        service_manager_address,
        service_handler_client.provider,
    ))
}

async fn handle_custom_submit(
    state: &HttpState,
    packet: &Packet,
    queue: &[QueuedPacket],
    submit_action: SubmitAction,
) -> AggregatorResult<TransactionReceipt> {
    let chain = ChainKey::new(submit_action.chain)?;
    let contract_address = Address::from_slice(&submit_action.contract_address.raw_bytes);

    let service_manager = get_submission_service_manager(state, &chain, contract_address).await?;

    let block_height_minus_one = service_manager
        .provider()
        .get_block_number()
        .await
        .map_err(|e| AggregatorError::BlockNumber(e.into()))?
        - 1;

    let signatures: Vec<EnvelopeSignature> = queue
        .iter()
        .map(|queued| queued.packet.signature.clone())
        .collect();

    let signature_data = packet
        .envelope
        .signature_data(signatures, block_height_minus_one)?;

    let result = service_manager
        .validate(
            packet.envelope.clone().into(),
            signature_data.clone().into(),
        )
        .call()
        .await;

    match result {
        Ok(_) => {
            tracing::info!("Service manager validation passed for custom submit");
        }
        Err(err) => match err.as_decoded_interface_error::<ServiceManagerError>() {
            Some(err) => {
                return Err(AggregatorError::ServiceManagerValidateKnown(err));
            }
            None => match err.as_revert_data() {
                Some(raw) => {
                    return Err(AggregatorError::ServiceManagerValidateAnyRevert(
                        raw.to_string(),
                    ))
                }
                None => return Err(AggregatorError::ServiceManagerValidateUnknown(err)),
            },
        },
    }

    let client = state.get_evm_client(&chain).await?;
    let tx_receipt = client
        .send_envelope_signatures(
            packet.envelope.clone(),
            signature_data,
            contract_address,
            None,
        )
        .await?;

    tracing::info!(
        "Custom submit transaction sent: {:?}",
        tx_receipt.transaction_hash
    );

    Ok(tx_receipt)
}

#[allow(clippy::manual_async_fn)]
fn handle_timer_callback(
    state: HttpState,
    packet: Packet,
    queue_id: QuorumQueueId,
    signer: Address,
    delay: wavs_types::Duration,
) -> impl std::future::Future<Output = ()> + Send + 'static {
    async move {
        tokio::time::sleep(delay.into()).await;

        tracing::info!(
            "Timer expired after {} seconds, executing callback",
            delay.secs
        );

        let component = match &packet.service.workflows[&packet.workflow_id].submit {
            wavs_types::Submit::Aggregator { component, .. } => component,
            _ => {
                tracing::error!("Failed to get aggregator component from workflow");
                return;
            }
        };

        let callback_actions = match state
            .aggregator_engine
            .execute_timer_callback(component, &packet)
            .await
        {
            Ok(actions) => actions,
            Err(e) => {
                tracing::error!("Timer callback execution failed: {}", e);
                return;
            }
        };

        for callback_action in callback_actions {
            let result = process_action(
                state.clone(),
                packet.clone(),
                queue_id.clone(),
                callback_action.clone(),
                signer,
            )
            .await;

            if let Err(e) = result {
                tracing::error!("Timer callback action processing failed: {:?}", e);
            }
        }
    }
}

fn add_packet_to_quorum_queue(
    packet: &Packet,
    mut queue: Vec<QueuedPacket>,
    signer: Address,
) -> AggregatorResult<Vec<QueuedPacket>> {
    match queue.first() {
        None => {}
        Some(prev) => {
            // check if the packet is the same as the last one
            // TODO - let custom logic here? wasm component?
            if packet.envelope != prev.packet.envelope {
                return Err(PacketValidationError::EnvelopeDiff.into());
            }
        }
    }

    for queued_packet in queue.iter_mut() {
        // if the signer is the same as the one in the queue, we can just update it
        // this effectively allows re-trying failed aggregation
        if signer == queued_packet.signer {
            *queued_packet = QueuedPacket {
                packet: packet.clone(),
                signer,
            };

            return Ok(queue);
        }
    }

    queue.push(QueuedPacket {
        packet: packet.clone(),
        signer,
    });

    Ok(queue)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{args::CliArgs, config::Config};
    use futures::{stream::FuturesUnordered, StreamExt};
    use std::{
        collections::{BTreeMap, HashSet},
        sync::{Arc, Mutex},
    };

    use alloy_primitives::Address;
    use alloy_provider::DynProvider;
    use utils::{
        config::{ConfigBuilder, EvmChainConfigBuilder},
        filesystem::workspace_path,
        test_utils::{
            address::rand_address_evm,
            middleware::{AvsOperator, MiddlewareInstance, MiddlewareServiceManagerConfig},
            mock_engine::COMPONENT_SIMPLE_AGGREGATOR_BYTES,
            mock_service_manager::MockServiceManager,
            test_contracts::{SimpleServiceHandlerInstance, TestContractDeps},
            test_packet::{mock_envelope, mock_packet, mock_signer, packet_from_service},
        },
    };
    use wavs_types::{ComponentDigest, Credential, Service, SignatureKind, WorkflowId};

    #[test]
    fn packet_validation() {
        let signer_1 = mock_signer();
        let signer_2 = mock_signer();
        let envelope_1 = mock_envelope(1, [1, 2, 3]);
        let envelope_2 = mock_envelope(2, [4, 5, 6]);

        let packet_1 = mock_packet(&signer_1, &envelope_1, "workflow-1".parse().unwrap());

        let derived_signer_1_address = packet_1
            .signature
            .evm_signer_address(&packet_1.envelope)
            .unwrap();
        assert_eq!(derived_signer_1_address, signer_1.address());

        // empty queue is okay
        let queue = add_packet_to_quorum_queue(&packet_1, Vec::new(), signer_1.address()).unwrap();

        // succeeds, replaces the packet for the signer
        let packet_2 = mock_packet(&signer_1, &envelope_1, "workflow-1".parse().unwrap());
        let queue =
            add_packet_to_quorum_queue(&packet_2, queue.clone(), signer_1.address()).unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].packet.signature.data, packet_2.signature.data);

        // "fails" (expectedly) because the envelope is different
        let packet_3 = mock_packet(&signer_2, &envelope_2, "workflow-1".parse().unwrap());
        add_packet_to_quorum_queue(&packet_3, queue.clone(), signer_2.address()).unwrap_err();

        // passes because the signer is different but envelope is the same
        let packet_3 = mock_packet(&signer_2, &envelope_1, "workflow-1".parse().unwrap());
        add_packet_to_quorum_queue(&packet_3, queue, signer_2.address()).unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn all_middleware_tests() {
        let middleware_instance = MiddlewareInstance::new().await.unwrap();

        let deps = TestDeps::new().await;
        // deploy all service manager serially
        let sm_many1 =
            MockServiceManager::new(middleware_instance.clone(), deps.contracts.client.clone())
                .await
                .unwrap();
        let sm_many2 =
            MockServiceManager::new(middleware_instance.clone(), deps.contracts.client.clone())
                .await
                .unwrap();
        let sm_mixed =
            MockServiceManager::new(middleware_instance.clone(), deps.contracts.client.clone())
                .await
                .unwrap();
        let sm_first =
            MockServiceManager::new(middleware_instance.clone(), deps.contracts.client.clone())
                .await
                .unwrap();

        // and all service handlers
        let sh_many1 = deps
            .contracts
            .deploy_simple_service_handler(sm_many1.address())
            .await;
        let sh_many2 = deps
            .contracts
            .deploy_simple_service_handler(sm_many2.address())
            .await;
        let sh_mixed = deps
            .contracts
            .deploy_simple_service_handler(sm_mixed.address())
            .await;
        let sh_first = deps
            .contracts
            .deploy_simple_service_handler(sm_first.address())
            .await;

        tokio::join!(
            async {
                println!("Running process_many_packets_serial...");
                process_many_packets(false, deps.clone(), sm_many1, sh_many1).await;
            },
            async {
                println!("Running process_many_packets_concurrent...");
                process_many_packets(true, deps.clone(), sm_many2, sh_many2).await;
            },
            async {
                println!("Running process_mixed_responses...");
                process_mixed_responses(deps.clone(), sm_mixed, sh_mixed).await;
            },
            async {
                println!("Running first_packet_sent...");
                first_packet_sent(deps.clone(), sm_first, sh_first).await;
            }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stress_test_storage_operations() {
        let deps = TestDeps::new().await;
        let state = deps.state;

        const NUM_CONCURRENT: usize = 300;
        const NUM_OPERATIONS_PER_TASK: usize = 3;

        let mut service_ids = Vec::new();
        for _ in 0..NUM_CONCURRENT {
            let manager = wavs_types::ServiceManager::Evm {
                chain: "evm:test-chain".parse().unwrap(),
                address: rand_address_evm(),
            };
            let service_id = wavs_types::ServiceId::from(&manager);
            state.register_service(&service_id).unwrap();
            service_ids.push(service_id);
        }

        let mut futures = FuturesUnordered::new();

        for (task_id, service_id) in service_ids.iter().enumerate() {
            futures.push({
                let state = state.clone();
                let service_id = service_id.clone();
                async move {
                    for op in 0..NUM_OPERATIONS_PER_TASK {
                        let registered = state.service_registered(service_id.clone()).await;
                        assert!(registered, "Service {} should be registered", task_id);

                        let envelope = mock_envelope(task_id as u64, [op as u8, 0, 0]);
                        let event_id = envelope.eventId.into();
                        let queue_id = QuorumQueueId {
                            event_id,
                            aggregator_action: wavs_types::AggregatorAction::Submit(
                                wavs_types::SubmitAction {
                                    chain: "evm:test-chain".to_string(),
                                    contract_address: vec![0u8; 20],
                                },
                            ),
                        };

                        let queue = state.get_quorum_queue(&queue_id).await.unwrap();
                        assert!(matches!(queue, QuorumQueue::Active(_)));

                        let test_queue = QuorumQueue::Active(vec![]);
                        state
                            .save_quorum_queue(&queue_id, test_queue)
                            .await
                            .unwrap();

                        let retrieved = state.get_quorum_queue(&queue_id).await.unwrap();
                        assert!(matches!(retrieved, QuorumQueue::Active(_)));
                    }

                    task_id
                }
            });
        }

        let mut completed = Vec::new();
        while let Some(task_id) = futures.next().await {
            completed.push(task_id);
        }

        assert_eq!(completed.len(), NUM_CONCURRENT);
        println!(
            "Successfully completed {} concurrent tasks with {} operations each",
            NUM_CONCURRENT, NUM_OPERATIONS_PER_TASK
        );
    }

    async fn process_mixed_responses(
        deps: TestDeps,
        service_manager: MockServiceManager,
        service_handler: SimpleServiceHandlerInstance<DynProvider>,
    ) {
        const NUM_SIGNERS: usize = 3;
        const NUM_THRESHOLD: usize = 2;

        let signers = (0..NUM_SIGNERS).map(|_| mock_signer()).collect::<Vec<_>>();

        let avs_operators = signers
            .iter()
            .map(|signer| AvsOperator::new(signer.address(), signer.address()))
            .collect::<Vec<_>>();
        service_manager
            .configure(&MiddlewareServiceManagerConfig::new(
                &avs_operators,
                NUM_THRESHOLD as u64,
            ))
            .await
            .unwrap();

        let envelope = mock_envelope(1, [1, 2, 3]);

        // Make sure we properly collect errors without actually erroring out
        let service = deps
            .create_service(
                "workflow-1".parse().unwrap(),
                service_manager.address(),
                vec![*service_handler.address()],
            )
            .await;
        deps.state.register_service(&service.id()).unwrap();

        let mut all_results = Vec::new();
        for signer in signers.iter().take(NUM_THRESHOLD) {
            let packet = packet_from_service(
                signer,
                &service,
                service.workflows.keys().next().unwrap(),
                &envelope,
            );
            let state = deps.state.clone();
            let results = process_packet(state.clone(), &packet).await.unwrap();
            all_results.push(results);
        }

        for (signer_index, final_results) in all_results.into_iter().enumerate() {
            assert_eq!(
                final_results.len(),
                1,
                "Should have exactly one response per packet"
            );
            let result = &final_results[0];

            match signer_index {
                // first signer is just aggregating
                0 => {
                    assert!(
                        matches!(result, AddPacketResponse::Aggregated { count: 1, .. }),
                        "First signer expected Aggregated {{ count: 1 }}, got {:?}",
                        result
                    );
                }
                // second signer sends (reaches threshold)
                1 => {
                    assert!(matches!(result, AddPacketResponse::Sent { count: 2, .. }));
                }
                // subsequent signers should see it's already sent (burned)
                n if n >= 2 => {
                    assert!(matches!(result, AddPacketResponse::Burned));
                }
                _ => {}
            }
        }

        // now try again, for the same envelope - should be similar except we get burn results
        let mut all_results = Vec::new();
        for signer in signers.iter().take(NUM_THRESHOLD) {
            let packet = packet_from_service(
                signer,
                &service,
                service.workflows.keys().next().unwrap(),
                &envelope,
            );
            let state = deps.state.clone();
            let results = process_packet(state.clone(), &packet).await.unwrap();
            all_results.push(results);
        }

        for (signer_index, final_results) in all_results.into_iter().enumerate() {
            assert_eq!(
                final_results.len(),
                1,
                "Should have exactly one response per packet"
            );
            let result = &final_results[0];

            // All packets should be burned since the envelope was already sent
            assert!(
                matches!(result, AddPacketResponse::Burned),
                "Signer {} expected Burned, got {:?}",
                signer_index,
                result
            );
        }

        let mut all_results = Vec::new();
        for signer in signers.iter().take(NUM_THRESHOLD) {
            let packet = packet_from_service(
                signer,
                &service,
                service.workflows.keys().next().unwrap(),
                &envelope,
            );
            let state = deps.state.clone();
            let results = process_packet(state.clone(), &packet).await.unwrap();
            all_results.push(results);
        }

        for (signer_index, final_results) in all_results.into_iter().enumerate() {
            for (agg_index, result) in final_results.into_iter().enumerate() {
                match (signer_index, agg_index) {
                    // valid chain is burned
                    (_, 0) => {
                        assert!(matches!(result, AddPacketResponse::Burned));
                    }
                    // first signer on previously-invalid chain still aggregates properly
                    (0, 1) => {
                        assert!(matches!(
                            result,
                            AddPacketResponse::Aggregated { count: 1, .. }
                        ));
                    }
                    // second signer on previously-invalid chain now sends properly!!
                    (1, 1) => {
                        assert!(matches!(result, AddPacketResponse::Sent { count: 2, .. }));
                    }
                    _ => {
                        panic!(
                            "Unexpected result for signer {} and aggregator {}: {:?}",
                            signer_index, agg_index, result
                        );
                    }
                }
            }
        }
    }

    async fn first_packet_sent(
        deps: TestDeps,
        service_manager: MockServiceManager,
        service_handler: SimpleServiceHandlerInstance<DynProvider>,
    ) {
        // Configure the service with a threshold of 1 (first packet sends immediately)
        let signer = mock_signer();

        let avs_operators = vec![AvsOperator::new(signer.address(), signer.address())];
        service_manager
            .configure(&MiddlewareServiceManagerConfig::new(&avs_operators, 1u64))
            .await
            .unwrap();

        let envelope = mock_envelope(1, [1, 2, 3]);
        let service = deps
            .create_service(
                "workflow-1".parse().unwrap(),
                service_manager.address(),
                vec![*service_handler.address()],
            )
            .await;
        deps.state.register_service(&service.id()).unwrap();

        let packet = packet_from_service(
            &signer,
            &service,
            service.workflows.keys().next().unwrap(),
            &envelope,
        );

        // First packet: should be validated and sent
        let responses = process_packet(deps.state.clone(), &packet).await.unwrap();
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            AddPacketResponse::Sent { count, .. } => {
                assert_eq!(*count, 1);
            }
            other => panic!("Expected Sent, got {:?}", other),
        }

        // Resend the same packet: should be Burned
        let responses = process_packet(deps.state.clone(), &packet).await.unwrap();
        assert_eq!(responses.len(), 1);
        assert!(matches!(responses[0], AddPacketResponse::Burned));
    }

    async fn process_many_packets(
        concurrent: bool,
        deps: TestDeps,
        service_manager: MockServiceManager,
        service_handler: SimpleServiceHandlerInstance<DynProvider>,
    ) {
        const NUM_SIGNERS: usize = 20;
        const NUM_THRESHOLD: usize = NUM_SIGNERS / 2 + 1;

        let signers = (0..NUM_SIGNERS).map(|_| mock_signer()).collect::<Vec<_>>();

        let avs_operators = signers
            .iter()
            .map(|signer| AvsOperator::new(signer.address(), signer.address()))
            .collect::<Vec<_>>();
        service_manager
            .configure(&MiddlewareServiceManagerConfig::new(
                &avs_operators,
                NUM_THRESHOLD as u64,
            ))
            .await
            .unwrap();

        let service = deps
            .create_service(
                "workflow-1".parse().unwrap(),
                service_manager.address(),
                vec![*service_handler.address()],
            )
            .await;
        deps.state.register_service(&service.id()).unwrap();

        let envelope = mock_envelope(1, [1, 2, 3]);

        let seen_count: Arc<Mutex<HashSet<usize>>> = Arc::new(Mutex::new(HashSet::new()));

        if !concurrent {
            for (index, signer) in signers.iter().enumerate() {
                let packet = packet_from_service(
                    signer,
                    &service,
                    service.workflows.keys().next().unwrap(),
                    &envelope,
                );
                let resp = process_packet(deps.state.clone(), &packet)
                    .await
                    .unwrap()
                    .pop()
                    .unwrap();
                match resp {
                    AddPacketResponse::Aggregated { count } => {
                        let mut seen_count = seen_count.lock().unwrap();
                        if !seen_count.insert(count) {
                            panic!("Duplicate count: {}", count);
                        }
                    }
                    AddPacketResponse::Sent {
                        count,
                        tx_receipt: _,
                    } => {
                        // in serial mode, break when we get a sent packet
                        // and assert that it's what we expect
                        assert_eq!(count, NUM_THRESHOLD);
                        assert_eq!(count - 1, index);
                        break;
                    }
                    AddPacketResponse::TimerStarted { delay_seconds: _ } => {}
                    AddPacketResponse::Error { reason } => {
                        panic!("{}", reason);
                    }
                    AddPacketResponse::Burned => {
                        panic!("should not get to burned, broke the loop upon sent");
                    }
                }
            }
        } else {
            let mut futures = FuturesUnordered::new();
            // in concurrent mode, just fire off exactly NUM_THRESHHOLD signers
            for signer in signers.iter().take(NUM_THRESHOLD) {
                let packet = packet_from_service(
                    signer,
                    &service,
                    service.workflows.keys().next().unwrap(),
                    &envelope,
                );
                futures.push({
                    let state = deps.state.clone();
                    let seen_count = seen_count.clone();
                    async move {
                        match process_packet(state, &packet).await.unwrap().pop().unwrap() {
                            AddPacketResponse::Aggregated { count } => {
                                let mut seen_count = seen_count.lock().unwrap();
                                if !seen_count.insert(count) {
                                    panic!("Duplicate count: {}", count);
                                }
                            }
                            AddPacketResponse::Sent { .. } => {}
                            AddPacketResponse::TimerStarted { .. } => {}
                            other => panic!("Unexpected response: {:?}", other),
                        }
                    }
                });
            }

            while futures.next().await.is_some() {
                // just wait for all futures to finish
            }
        }

        // last one should be burned
        let packet = packet_from_service(
            signers.last().unwrap(),
            &service,
            service.workflows.keys().next().unwrap(),
            &envelope,
        );
        let responses = process_packet(deps.state.clone(), &packet).await.unwrap();
        for resp in responses {
            assert!(matches!(resp, AddPacketResponse::Burned));
        }
    }

    async fn mock_service(
        chain: ChainKey,
        workflow_id: WorkflowId,
        service_manager_address: Address,
        service_handler_addresses: Vec<Address>,
        aggregator_digest: ComponentDigest,
    ) -> wavs_types::Service {
        let mut component =
            wavs_types::Component::new(wavs_types::ComponentSource::Digest(aggregator_digest));
        component
            .config
            .insert("chain".to_string(), chain.to_string());
        // SimpleAggregator needs the service handler address
        if !service_handler_addresses.is_empty() {
            component.config.insert(
                "service_handler".to_string(),
                service_handler_addresses[0].to_string(),
            );
        }

        mock_service_with_submit(
            chain,
            workflow_id,
            service_manager_address,
            wavs_types::Submit::Aggregator {
                url: "http://localhost:8080".to_string(),
                component: Box::new(component),
                signature_kind: SignatureKind::evm_default(),
            },
        )
        .await
    }

    async fn mock_service_with_submit(
        chain: ChainKey,
        workflow_id: WorkflowId,
        service_manager_address: Address,
        submit: wavs_types::Submit,
    ) -> wavs_types::Service {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            workflow_id,
            wavs_types::Workflow {
                trigger: wavs_types::Trigger::Manual,
                component: wavs_types::Component::new(wavs_types::ComponentSource::Digest(
                    wavs_types::ComponentDigest::hash([0; 32]),
                )),
                submit,
            },
        );

        wavs_types::Service {
            name: "service".to_string(),
            status: wavs_types::ServiceStatus::Active,
            workflows,
            manager: wavs_types::ServiceManager::Evm {
                chain,
                address: service_manager_address,
            },
        }
    }

    #[derive(Clone)]
    struct TestDeps {
        contracts: Arc<TestContractDeps>,
        state: HttpState,
        simple_aggregator_digest: ComponentDigest,
    }

    impl TestDeps {
        async fn new() -> Self {
            let contract_deps = Arc::new(TestContractDeps::new().await);

            let data_dir = tempfile::tempdir().unwrap();
            let mut config: Config = ConfigBuilder::new(CliArgs {
                data: Some(data_dir.path().to_path_buf()),
                home: Some(workspace_path()),
                // deliberately point to a non-existing file
                dotenv: Some(tempfile::NamedTempFile::new().unwrap().path().to_path_buf()),
                ..Default::default()
            })
            .build()
            .unwrap();

            // Use the same chain configuration from contract_deps
            config.chains.evm.insert(
                contract_deps.chain.id.clone(),
                EvmChainConfigBuilder {
                    http_endpoint: Some(contract_deps._anvil.endpoint()),
                    ws_endpoint: Some(contract_deps._anvil.ws_endpoint()),
                    faucet_endpoint: None,
                    poll_interval_ms: None,
                },
            );

            config.credential = Some(Credential::new(
                "test test test test test test test test test test test junk".to_string(),
            ));

            let state = HttpState::new_with_engine(config).unwrap();

            let digest = state
                .aggregator_engine
                .upload_component(COMPONENT_SIMPLE_AGGREGATOR_BYTES.to_vec())
                .await
                .unwrap();

            Self {
                contracts: contract_deps,
                state,
                simple_aggregator_digest: digest,
            }
        }

        pub async fn create_service(
            &self,
            workflow_id: WorkflowId,
            service_manager_address: Address,
            service_handler_addresses: Vec<Address>,
        ) -> Service {
            mock_service(
                self.contracts.chain.clone(),
                workflow_id,
                service_manager_address,
                service_handler_addresses,
                self.simple_aggregator_digest.clone(),
            )
            .await
        }
    }
}
