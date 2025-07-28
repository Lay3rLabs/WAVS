use alloy_primitives::Address;
use alloy_provider::{DynProvider, Provider};
use alloy_rpc_types_eth::TransactionReceipt;
use axum::{extract::State, response::IntoResponse, Json};
use tracing::instrument;
use utils::async_transaction::AsyncTransaction;
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Aggregator, ChainName, EnvelopeExt, EnvelopeSignature, EvmContractSubmission,
    IWavsServiceHandler::IWavsServiceHandlerInstance,
    IWavsServiceManager::IWavsServiceManagerInstance,
    Packet, ServiceManagerError,
};

use crate::{
    engine::{AggregatorAction, SubmitAction},
    error::{AggregatorError, AggregatorResult, PacketValidationError},
    http::{
        error::AnyError,
        state::{HttpState, PacketQueue, PacketQueueId, QueuedPacket},
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
#[instrument(level = "info", skip(state, req), fields(service_id = %req.packet.service.id(), workflow_id = %req.packet.workflow_id))]
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

#[instrument(level = "debug", skip(state, packet), fields(service_id = %packet.service.id(), workflow_id = %packet.workflow_id))]
async fn process_packet(
    state: HttpState,
    packet: &Packet,
) -> AggregatorResult<Vec<AddPacketResponse>> {
    if !state.service_registered(&packet.service.id()) {
        return Err(AggregatorError::MissingService(packet.service.id()));
    }
    let event_id = packet.event_id();

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
        let service_manager_client = state
            .get_evm_client(packet.service.manager.chain_name())
            .await?;
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

    let resp = AggregatorProcess {
        state: &state,
        async_tx: state.queue_transaction.clone(),
        packet,
        signer,
    }
    .run()
    .await;

    match resp {
        Ok(resp) => Ok(vec![resp]),
        Err(e) => Ok(vec![AddPacketResponse::Error {
            reason: format!("{:?}", e),
        }]),
    }
}

struct AggregatorProcess<'a> {
    state: &'a HttpState,
    async_tx: AsyncTransaction<PacketQueueId>,
    packet: &'a Packet,
    signer: Address,
}

impl AggregatorProcess<'_> {
    #[instrument(level = "debug", skip(self), fields(signer = ?self.signer))]
    async fn run(self) -> AggregatorResult<AddPacketResponse> {
        let Self {
            state,
            async_tx,
            packet,
            signer,
        } = self;

        match aggregator {
            Aggregator::Evm(EvmContractSubmission {
                chain_name: _,
                address: _,
                max_gas,
            }) => {
                // execute the logic within a transaction, keyed by queue_id
                // other queue ids can run concurrently, but this makes sure that
                // we aren't validating a queue that was updated from another request coming in
                async_tx
                    .run(queue_id.clone(), move || async move {
                        let queue = match state.get_packet_queue(&queue_id)? {
                            PacketQueue::Alive(queue) => {
                                if let (Some(engine), wavs_types::Submit::Aggregator { component, .. }) =
                                    (&state.aggregator_engine, &packet.service.workflows[&packet.workflow_id].submit)
                                {
                                    match engine.execute_packet(&component, packet).await {
                                        Ok(actions) => {
                                            let updated_queue = process_aggregator_actions(state, packet, queue, signer, actions).await?;
                                            updated_queue
                                        },
                                        Err(e) => {
                                            tracing::error!("Custom aggregator component failed: {}", e);
                                            return Err(AggregatorError::ComponentExecution(e.to_string()));
                                        }
                                    }
                                } else {
                                    add_packet_to_queue(packet, queue, signer)?
                                }
                            }
                            PacketQueue::Burned => {
                                return Ok(AddPacketResponse::Burned);
                            }
                        };

                        let (chain_name, address) = match aggregator {
                            Aggregator::Evm(EvmContractSubmission { chain_name, address, .. }) => {
                                (chain_name, address)
                            }
                        };
                        let service_manager = get_submission_service_manager(state, chain_name, *address).await?;

                        // TODO: anvil specific (blockheight -1)? InvalidReferenceBlock(). ECDSA logic error / fixed in BLS?
                        let block_height_minus_one = service_manager
                            .provider()
                            .get_block_number()
                            .await
                            .map_err(|e| AggregatorError::BlockNumber(e.into()))? - 1;

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
                                let client = state.get_evm_client(chain_name).await?;
                                tracing::info!(
                                    "Sending aggregated packet to chain: {}, address: {:?}, block_height: {}",
                                    chain_name,
                                    address,
                                    block_height_minus_one
                                );
                                let tx_receipt = client
                                    .send_envelope_signatures(
                                        packet.envelope.clone(),
                                        signature_data.clone(),
                                        *address,
                                        *max_gas,
                                    )
                                    .await?;
                                tracing::info!(
                                    "Transaction sent successfully: {:?}",
                                    tx_receipt.transaction_hash
                                );

                                state.save_packet_queue(&queue_id, PacketQueue::Burned)?;
                                tracing::info!("Packet queue burned after successful submission");

                                Ok(AddPacketResponse::Sent {
                                    tx_receipt: Box::new(tx_receipt),
                                    count: queue.len(),
                                })
                            },
                            Err(err) => {
                                match err.as_decoded_interface_error::<ServiceManagerError>() {
                                    Some(ServiceManagerError::InsufficientQuorum(_)) => {
                                        // insufficient quorum means we just keep aggregating
                                        state.save_packet_queue(
                                            &queue_id,
                                            PacketQueue::Alive(queue.clone()),
                                        )?;

                                        Ok(AddPacketResponse::Aggregated { count: queue.len() })
                                    },
                                    Some(err) => {
                                        Err(AggregatorError::ServiceManagerValidateKnown(err))
                                    }
                                    None => {
                                        match err.as_revert_data() {
                                            Some(raw) => Err(AggregatorError::ServiceManagerValidateAnyRevert(raw.to_string())),
                                            None => Err(AggregatorError::ServiceManagerValidateUnknown(err))
                                        }
                                    }
                                }
                            }
                        }
                    })
                    .await
            }
        }
    }
}

async fn get_submission_service_manager(
    state: &HttpState,
    chain_name: &ChainName,
    service_handler_address: Address,
) -> AggregatorResult<IWavsServiceManagerInstance<DynProvider>> {
    // we need to get the service manager from the perspective of the service handler
    // which may be different than the service manager where the operator is staked
    // e.g. in the case of operator sets that are mirrored across multiple chains
    let service_handler_client = state.get_evm_client(chain_name).await?;
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

async fn process_aggregator_actions(
    state: &HttpState,
    packet: &Packet,
    mut queue: Vec<QueuedPacket>,
    signer: Address,
    actions: Vec<AggregatorAction>,
) -> AggregatorResult<Vec<QueuedPacket>> {
    tracing::info!(
        "Custom aggregator component returned {} actions",
        actions.len()
    );

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

    if actions.is_empty() {
        tracing::debug!("Component returned no actions");
    } else {
        for action in actions {
            match action {
                AggregatorAction::Submit(submit_action) => {
                    tracing::info!(
                        "Component requested submit to chain: {}, contract: {:?}",
                        submit_action.chain_name,
                        submit_action.contract_address
                    );

                    match handle_custom_submit(state, packet, &queue, submit_action).await {
                        Ok(_) => {
                            tracing::info!("Custom submit completed successfully");
                        }
                        Err(e) => {
                            tracing::error!("Custom submit failed: {}", e);
                        }
                    }
                }
                AggregatorAction::Timer(timer_action) => {
                    tracing::info!(
                        "Component requested timer callback in {} seconds",
                        timer_action.delay
                    );
                    todo!("Implement timer scheduling system");
                }
            }
        }
    }

    Ok(queue)
}

async fn handle_custom_submit(
    state: &HttpState,
    packet: &Packet,
    queue: &[QueuedPacket],
    submit_action: SubmitAction,
) -> AggregatorResult<TransactionReceipt> {
    let chain_name = ChainName::new(submit_action.chain_name)?;
    let contract_address = Address::from_slice(&submit_action.contract_address.raw_bytes);

    let client = state.get_evm_client(&chain_name).await?;

    let service_handler =
        IWavsServiceHandlerInstance::new(contract_address, client.provider.clone());
    let service_manager_address = service_handler
        .getServiceManager()
        .call()
        .await
        .map_err(AggregatorError::ServiceManagerLookup)?;

    let service_manager =
        IWavsServiceManagerInstance::new(service_manager_address, client.provider.clone());

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

    let service_manager =
        get_submission_service_manager(state, &chain_name, contract_address).await?;
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
        Err(err) => {
            match err.as_decoded_interface_error::<ServiceManagerError>() {
                Some(ServiceManagerError::InsufficientQuorum(quorum_err)) => {
                    // insufficient quorum - in custom submit this is an error, not "keep aggregating"
                    return Err(AggregatorError::ServiceManagerValidateKnown(
                        ServiceManagerError::InsufficientQuorum(quorum_err),
                    ));
                }
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
            }
        }
    }

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

fn add_packet_to_queue(
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
    use alloy_primitives::U256;
    use futures::{stream::FuturesUnordered, StreamExt};
    use std::{
        collections::{BTreeMap, HashSet},
        sync::{Arc, Mutex},
    };

    use utils::{
        config::{ConfigBuilder, EvmChainConfig},
        filesystem::workspace_path,
        test_utils::{
            test_contracts::TestContractDeps,
            test_packet::{mock_envelope, mock_packet, mock_signer, packet_from_service},
        },
    };
    use wavs_types::{ChainName, Service, WorkflowID};

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
        let queue = add_packet_to_queue(&packet_1, Vec::new(), signer_1.address()).unwrap();

        // succeeds, replaces the packet for the signer
        let packet_2 = mock_packet(&signer_1, &envelope_1, "workflow-1".parse().unwrap());
        let queue = add_packet_to_queue(&packet_2, queue.clone(), signer_1.address()).unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(
            queue[0].packet.signature.as_bytes(),
            packet_2.signature.as_bytes()
        );

        // "fails" (expectedly) because the envelope is different
        let packet_3 = mock_packet(&signer_2, &envelope_2, "workflow-1".parse().unwrap());
        add_packet_to_queue(&packet_3, queue.clone(), signer_2.address()).unwrap_err();

        // passes because the signer is different but envelope is the same
        let packet_3 = mock_packet(&signer_2, &envelope_1, "workflow-1".parse().unwrap());
        add_packet_to_queue(&packet_3, queue, signer_2.address()).unwrap();
    }

    #[tokio::test]
    async fn process_many_packets_serial() {
        process_many_packets(false).await;
    }

    #[tokio::test]
    async fn process_many_packets_concurrent() {
        process_many_packets(true).await;
    }

    #[tokio::test]
    async fn process_mixed_responses() {
        let deps = TestDeps::new().await;

        let service_manager = deps.contracts.deploy_simple_service_manager().await;

        let mut signers = Vec::new();
        const NUM_SIGNERS: usize = 3;
        const NUM_THRESHOLD: usize = 2;

        service_manager
            .setLastCheckpointTotalWeight(U256::from(NUM_SIGNERS as u64))
            .send()
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();

        service_manager
            .setLastCheckpointThresholdWeight(U256::from(NUM_THRESHOLD as u64))
            .send()
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();

        for _ in 0..NUM_SIGNERS {
            let signer = mock_signer();
            service_manager
                .setOperatorWeight(signer.address(), U256::ONE)
                .send()
                .await
                .unwrap()
                .watch()
                .await
                .unwrap();
            signers.push(signer);
        }

        let envelope = mock_envelope(1, [1, 2, 3]);

        let service_handler = deps
            .contracts
            .deploy_simple_service_handler(*service_manager.address())
            .await;

        let fixed_second_service_handler = deps
            .contracts
            .deploy_simple_service_handler(*service_manager.address())
            .await;

        // Make sure we properly collect errors without actually erroring out
        let mut service = deps
            .create_service(
                "workflow-1".parse().unwrap(),
                *service_manager.address(),
                vec![*service_handler.address(), Address::ZERO],
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
            for (agg_index, result) in final_results.into_iter().enumerate() {
                match (signer_index, agg_index) {
                    // invalid chain errors
                    (_, 1) => {
                        assert!(matches!(result, AddPacketResponse::Error { .. }));
                    }
                    // first signer on valid chain is just aggregating
                    (0, 0) => {
                        assert!(matches!(
                            result,
                            AddPacketResponse::Aggregated { count: 1, .. }
                        ));
                    }
                    // second signer on valid chain sends
                    (1, 0) => {
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
            for (agg_index, result) in final_results.into_iter().enumerate() {
                match (signer_index, agg_index) {
                    // valid chain is burned
                    (_, 0) => {
                        assert!(matches!(result, AddPacketResponse::Burned));
                    }
                    // invalid chain errors
                    (_, 1) => {
                        assert!(matches!(result, AddPacketResponse::Error { .. }));
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

        // now let's change reality, make the second aggregator valid
        // we should get essentially the same as previous attempt, but second aggregator should succeed

        if let wavs_types::Submit::Aggregator {
            evm_contracts: Some(ref mut contracts),
            ..
        } = &mut service.workflows.iter_mut().next().unwrap().1.submit
        {
            if let Some(contract) = contracts.get_mut(1) {
                *contract = wavs_types::EvmContractSubmission {
                    chain_name: deps.contracts.chain_name.clone(),
                    address: *fixed_second_service_handler.address(),
                    max_gas: None,
                };
            }
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

    #[tokio::test]
    async fn first_packet_sent() {
        let deps = TestDeps::new().await;

        let service_manager = deps.contracts.deploy_simple_service_manager().await;
        let service_handler = deps
            .contracts
            .deploy_simple_service_handler(*service_manager.address())
            .await;

        // Configure the service with a threshold of 1 (first packet sends immediately)
        service_manager
            .setLastCheckpointTotalWeight(U256::ONE)
            .send()
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();

        service_manager
            .setLastCheckpointThresholdWeight(U256::ONE)
            .send()
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();

        let signer = mock_signer();
        service_manager
            .setOperatorWeight(signer.address(), U256::ONE)
            .send()
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();

        let envelope = mock_envelope(1, [1, 2, 3]);
        let service = deps
            .create_service(
                "workflow-1".parse().unwrap(),
                *service_manager.address(),
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

    async fn process_many_packets(concurrent: bool) {
        let deps = TestDeps::new().await;

        let service_manager = deps.contracts.deploy_simple_service_manager().await;
        let service_handler = deps
            .contracts
            .deploy_simple_service_handler(*service_manager.address())
            .await;
        let service = deps
            .create_service(
                "workflow-1".parse().unwrap(),
                *service_manager.address(),
                vec![*service_handler.address()],
            )
            .await;
        deps.state.register_service(&service.id()).unwrap();

        let mut signers = Vec::new();
        const NUM_SIGNERS: usize = 20;
        const NUM_THRESHOLD: usize = NUM_SIGNERS / 2 + 1;

        service_manager
            .setLastCheckpointTotalWeight(U256::from(NUM_SIGNERS as u64))
            .send()
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();

        service_manager
            .setLastCheckpointThresholdWeight(U256::from(NUM_THRESHOLD as u64))
            .send()
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();

        for _ in 0..NUM_SIGNERS {
            let signer = mock_signer();
            service_manager
                .setOperatorWeight(signer.address(), U256::ONE)
                .send()
                .await
                .unwrap()
                .watch()
                .await
                .unwrap();
            signers.push(signer);
        }

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
                        if let AddPacketResponse::Aggregated { count } =
                            process_packet(state, &packet).await.unwrap().pop().unwrap()
                        {
                            let mut seen_count = seen_count.lock().unwrap();
                            if !seen_count.insert(count) {
                                panic!("Duplicate count: {}", count);
                            }
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
        chain_name: ChainName,
        workflow_id: WorkflowID,
        service_manager_address: Address,
        service_handler_addresses: Vec<Address>,
    ) -> wavs_types::Service {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            workflow_id,
            wavs_types::Workflow {
                trigger: wavs_types::Trigger::Manual,
                component: wavs_types::Component::new(wavs_types::ComponentSource::Digest(
                    wavs_types::ComponentDigest::hash([0; 32]),
                )),
                submit: wavs_types::Submit::Aggregator {
                    url: "http://dummy".to_string(),
                    component: None,
                    evm_contracts: Some(
                        service_handler_addresses
                            .into_iter()
                            .map(|address| wavs_types::EvmContractSubmission {
                                chain_name: chain_name.clone(),
                                address,
                                max_gas: None,
                            })
                            .collect(),
                    ),
                },
            },
        );

        wavs_types::Service {
            name: "service".to_string(),
            status: wavs_types::ServiceStatus::Active,
            workflows,
            manager: wavs_types::ServiceManager::Evm {
                chain_name,
                address: service_manager_address,
            },
        }
    }

    struct TestDeps {
        contracts: TestContractDeps,
        state: HttpState,
    }

    impl TestDeps {
        async fn new() -> Self {
            let contract_deps = TestContractDeps::new().await;

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
                contract_deps.chain_name.clone(),
                EvmChainConfig {
                    chain_id: "31337".to_string(),
                    http_endpoint: Some(contract_deps._anvil.endpoint()),
                    ws_endpoint: Some(contract_deps._anvil.ws_endpoint()),
                    faucet_endpoint: None,
                    poll_interval_ms: None,
                },
            );

            config.credential =
                Some("test test test test test test test test test test test junk".to_string());

            let state = HttpState::new(config).unwrap();

            Self {
                contracts: contract_deps,
                state,
            }
        }

        pub async fn create_service(
            &self,
            workflow_id: WorkflowID,
            service_manager_address: Address,
            service_handler_addresses: Vec<Address>,
        ) -> Service {
            mock_service(
                self.contracts.chain_name.clone(),
                workflow_id,
                service_manager_address,
                service_handler_addresses,
            )
            .await
        }
    }
}
