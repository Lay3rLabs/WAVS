use alloy_primitives::Address;
use alloy_provider::{DynProvider, Provider};
use alloy_sol_types::SolError;
use axum::{extract::State, response::IntoResponse, Json};
use utils::async_transaction::AsyncTransaction;
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Aggregator, EnvelopeExt, EnvelopeSignature, EvmContractSubmission,
    IWavsServiceManager::{self, IWavsServiceManagerInstance},
    Packet,
};

use crate::{
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

async fn process_packet(
    state: HttpState,
    packet: &Packet,
) -> AggregatorResult<Vec<AddPacketResponse>> {
    let event_id = packet.event_id();
    let route = packet.route.clone();

    let service = state.get_service(&packet.route)?;
    let aggregators = &service.workflows[&packet.route.workflow_id].aggregators;

    if aggregators.is_empty() {
        return Err(AggregatorError::MissingWorkflow {
            workflow_id: route.workflow_id,
            service_id: route.service_id,
        });
    }

    // this implicitly validates that the signature is valid
    let signer = packet.signature.evm_signer_address(&packet.envelope)?;

    let service_manager_client = state.get_evm_client(service.manager.chain_name()).await?;
    let service_manager = IWavsServiceManager::new(
        service.manager.evm_address_unchecked(),
        service_manager_client.provider.clone(),
    );

    let mut responses: Vec<AddPacketResponse> = Vec::new();

    for (aggregator_index, aggregator) in aggregators.iter().enumerate() {
        let resp = AggregatorProcess {
            state: &state,
            service_manager: &service_manager,
            async_tx: state.queue_transaction.clone(),
            aggregator,
            queue_id: PacketQueueId {
                event_id: event_id.clone(),
                service_id: service.id.clone(),
                aggregator_index,
            },
            packet,
            signer,
        }
        .run()
        .await;

        match resp {
            Ok(resp) => {
                responses.push(resp);
            }
            Err(e) => {
                responses.push(AddPacketResponse::Error {
                    reason: format!("{:?}", e),
                });
            }
        }
    }

    if responses.len() != aggregators.len() {
        return Err(AggregatorError::UnexpectedResponsesLength {
            responses: responses.len(),
            aggregators: aggregators.len(),
        });
    }

    Ok(responses)
}

struct AggregatorProcess<'a> {
    state: &'a HttpState,
    async_tx: AsyncTransaction<PacketQueueId>,
    service_manager: &'a IWavsServiceManagerInstance<DynProvider>,
    aggregator: &'a Aggregator,
    queue_id: PacketQueueId,
    packet: &'a Packet,
    signer: Address,
}

impl AggregatorProcess<'_> {
    async fn run(self) -> AggregatorResult<AddPacketResponse> {
        let Self {
            state,
            async_tx,
            service_manager,
            aggregator,
            queue_id,
            packet,
            signer,
        } = self;

        match aggregator {
            Aggregator::Evm(EvmContractSubmission {
                chain_name,
                address,
                max_gas,
            }) => {
                // execute the logic within a transaction, keyed by queue_id
                // other queue ids can run concurrently, but this makes sure that
                // we aren't validating a queue that was updated from another request coming in
                async_tx
                    .run(queue_id.clone(), move || async move {
                        let queue = match state.get_packet_queue(&queue_id)? {
                            PacketQueue::Alive(queue) => {
                                // this will also locally validate the packet
                                add_packet_to_queue(packet, queue, signer)?
                            }
                            PacketQueue::Burned => {
                                return Ok(AddPacketResponse::Burned);
                            }
                        };

                        let block_height = service_manager
                            .provider()
                            .get_block_number()
                            .await
                            .map_err(|e| AggregatorError::BlockNumber(e.into()))?;

                        let signatures: Vec<EnvelopeSignature> = queue
                            .iter()
                            .map(|queued| queued.packet.signature.clone())
                            .collect();

                        // TODO: anvil specific (blockheight -1)? InvalidReferenceBlock(). ECDSA logic error / fixed in BLS?
                        let signature_data = packet
                            .envelope
                            .signature_data(signatures, block_height - 1)?;

                        // validate the potential quorum on-chain
                        // we'll get an error if quorum is not met, but may get other errors as well
                        // success means we've reached quorum and can send the signatures + data
                        match service_manager
                            .validate(
                                packet.envelope.clone().into(),
                                signature_data.clone().into(),
                            )
                            .call()
                            .await
                        {
                            Ok(_) => {
                                let client = state.get_evm_client(chain_name).await?;
                                let tx_receipt = client
                                    .send_envelope_signatures(
                                        packet.envelope.clone(),
                                        signature_data.clone(),
                                        *address,
                                        *max_gas,
                                    )
                                    .await?;

                                state.save_packet_queue(&queue_id, PacketQueue::Burned)?;

                                Ok(AddPacketResponse::Sent {
                                    tx_receipt: Box::new(tx_receipt),
                                    count: queue.len(),
                                })
                            }
                            Err(e) => {
                                if let Some(revert) = e
                                    .as_revert_data()
                                    .and_then(|raw| alloy_sol_types::Revert::abi_decode(&raw).ok())
                                {
                                    // TODO - we want to get the specific error of "valid but not enough signers"
                                    // but for now, we've validated the signature and other things locally
                                    // so we can be optimistic and aggregate
                                    tracing::debug!(
                                        "Aggregator {} validation failed: {}",
                                        chain_name,
                                        revert.reason
                                    );

                                    state.save_packet_queue(
                                        &queue_id,
                                        PacketQueue::Alive(queue.clone()),
                                    )?;

                                    Ok(AddPacketResponse::Aggregated { count: queue.len() })
                                } else {
                                    Err(AggregatorError::ServiceManagerValidate(e))
                                }
                            }
                        }
                    })
                    .await
            }
        }
    }
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
    use alloy_node_bindings::{Anvil, AnvilInstance};
    use alloy_primitives::{Bytes, FixedBytes, U256};
    use alloy_provider::DynProvider;
    use alloy_signer::{k256::ecdsa::SigningKey, SignerSync};
    use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
    use alloy_sol_types::SolValue;
    use futures::{stream::FuturesUnordered, StreamExt};
    use service_handler::{
        ISimpleSubmit::DataWithId,
        SimpleSubmit::{
            self as SimpleServiceHandler, SimpleSubmitInstance as SimpleServiceHandlerInstance,
        },
    };
    use service_manager::SimpleServiceManager::{self, SimpleServiceManagerInstance};
    use std::{
        collections::{BTreeMap, HashSet},
        sync::{Arc, Mutex},
    };
    use tempfile::TempDir;
    use utils::{
        config::{ConfigBuilder, EvmChainConfig},
        evm_client::EvmSigningClient,
        filesystem::workspace_path,
    };
    use wavs_types::{
        ChainName, Envelope, EnvelopeExt, EnvelopeSignature, PacketRoute, Service, ServiceID,
    };

    mod service_manager {
        use alloy_sol_types::sol;

        sol!(
            #[allow(missing_docs)]
            #[sol(rpc)]
            SimpleServiceManager,
            "../../examples/contracts/solidity/abi/SimpleServiceManager.sol/SimpleServiceManager.json"
        );
    }

    mod service_handler {
        use alloy_sol_types::sol;

        sol!(
            #[allow(missing_docs)]
            #[sol(rpc)]
            SimpleSubmit,
            "../../examples/contracts/solidity/abi/SimpleSubmit.sol/SimpleSubmit.json"
        );
    }

    #[test]
    fn packet_validation() {
        let signer_1 = mock_signer();
        let signer_2 = mock_signer();
        let envelope_1 = mock_envelope(1, [1, 2, 3]);
        let envelope_2 = mock_envelope(2, [4, 5, 6]);

        let packet_1 = mock_packet(&signer_1, &envelope_1, "service-1".parse().unwrap());

        let derived_signer_1_address = packet_1
            .signature
            .evm_signer_address(&packet_1.envelope)
            .unwrap();
        assert_eq!(derived_signer_1_address, signer_1.address());

        // empty queue is okay
        let queue = add_packet_to_queue(&packet_1, Vec::new(), signer_1.address()).unwrap();

        // succeeds, replaces the packet for the signer
        let packet_2 = mock_packet(&signer_1, &envelope_1, "service-1".parse().unwrap());
        let queue = add_packet_to_queue(&packet_2, queue.clone(), signer_1.address()).unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(
            queue[0].packet.signature.as_bytes(),
            packet_2.signature.as_bytes()
        );

        // "fails" (expectedly) because the envelope is different
        let packet_3 = mock_packet(&signer_2, &envelope_2, "service-1".parse().unwrap());
        add_packet_to_queue(&packet_3, queue.clone(), signer_2.address()).unwrap_err();

        // passes because the signer is different but envelope is the same
        let packet_3 = mock_packet(&signer_2, &envelope_1, "service-1".parse().unwrap());
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

        let service_manager = deps.deploy_simple_service_manager().await;

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
            .deploy_simple_service_handler(*service_manager.address())
            .await;

        let fixed_second_service_handler = deps
            .deploy_simple_service_handler(*service_manager.address())
            .await;

        // Make sure we properly collect errors without actually erroring out
        let mut service = deps
            .create_service(
                "service-1".parse().unwrap(),
                *service_manager.address(),
                vec![*service_handler.address(), Address::ZERO],
            )
            .await;

        let mut all_results = Vec::new();
        for signer in signers.iter().take(NUM_THRESHOLD) {
            let packet = mock_packet(signer, &envelope, service.id.clone());
            let state = deps.state.clone();
            let results = process_packet(state.clone(), &packet).await.unwrap();
            all_results.push(results);
        }

        for (signer_index, final_results) in all_results.into_iter().enumerate() {
            for (agg_index, result) in final_results.into_iter().enumerate() {
                match (signer_index, agg_index) {
                    // first signer on any chain is just aggregating
                    (0, _) => {
                        assert!(matches!(
                            result,
                            AddPacketResponse::Aggregated { count: 1, .. }
                        ));
                    }
                    // second signer on valid chain sends
                    (1, 0) => {
                        assert!(matches!(result, AddPacketResponse::Sent { count: 2, .. }));
                    }
                    // second signer on invalid chain errors
                    (1, 1) => {
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

        // now try again, for the same envelope - should be similar except we get burn results
        let mut all_results = Vec::new();
        for signer in signers.iter().take(NUM_THRESHOLD) {
            let packet = mock_packet(signer, &envelope, service.id.clone());
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
                    // first signer on invalid chain still aggregates properly
                    (0, 1) => {
                        assert!(matches!(
                            result,
                            AddPacketResponse::Aggregated { count: 1, .. }
                        ));
                    }
                    // second signer on invalid chain errors
                    (1, 1) => {
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

        *service
            .workflows
            .get_mut(&"workflow".parse().unwrap())
            .unwrap()
            .aggregators
            .get_mut(1)
            .unwrap() = wavs_types::Aggregator::Evm(wavs_types::EvmContractSubmission {
            chain_name: deps.chain_name.clone(),
            address: *fixed_second_service_handler.address(),
            max_gas: None,
        });

        deps.state.unchecked_save_service(&service).unwrap();

        let mut all_results = Vec::new();
        for signer in signers.iter().take(NUM_THRESHOLD) {
            let packet = mock_packet(signer, &envelope, service.id.clone());
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

    async fn process_many_packets(concurrent: bool) {
        let deps = TestDeps::new().await;

        let service_manager = deps.deploy_simple_service_manager().await;
        let service_handler = deps
            .deploy_simple_service_handler(*service_manager.address())
            .await;
        let service = deps
            .create_service(
                "service-2".parse().unwrap(),
                *service_manager.address(),
                vec![*service_handler.address()],
            )
            .await;

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
                let packet = mock_packet(signer, &envelope, service.id.clone());
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
                let packet = mock_packet(signer, &envelope, service.id.clone());
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
        let packet = mock_packet(signers.last().unwrap(), &envelope, service.id.clone());
        let responses = process_packet(deps.state.clone(), &packet).await.unwrap();
        for resp in responses {
            assert!(matches!(resp, AddPacketResponse::Burned));
        }
    }

    async fn mock_service(
        chain_name: ChainName,
        service_id: ServiceID,
        service_manager_address: Address,
        service_handler_addresses: Vec<Address>,
    ) -> wavs_types::Service {
        let mut workflows = BTreeMap::new();
        workflows.insert(
            "workflow".parse().unwrap(),
            wavs_types::Workflow {
                trigger: wavs_types::Trigger::Manual,
                component: wavs_types::Component::new(wavs_types::ComponentSource::Digest(
                    wavs_types::Digest::new(&[0; 32]),
                )),
                submit: wavs_types::Submit::None,
                aggregators: service_handler_addresses
                    .into_iter()
                    .map(|address| {
                        wavs_types::Aggregator::Evm(wavs_types::EvmContractSubmission {
                            chain_name: chain_name.clone(),
                            address,
                            max_gas: None,
                        })
                    })
                    .collect(),
            },
        );

        wavs_types::Service {
            id: service_id,
            name: "service".to_string(),
            status: wavs_types::ServiceStatus::Active,
            workflows,
            manager: wavs_types::ServiceManager::Evm {
                chain_name,
                address: service_manager_address,
            },
        }
    }
    fn mock_packet(
        signer: &LocalSigner<SigningKey>,
        envelope: &Envelope,
        service_id: ServiceID,
    ) -> Packet {
        let signature = signer.sign_hash_sync(&envelope.eip191_hash()).unwrap();

        Packet {
            envelope: envelope.clone(),
            route: PacketRoute {
                service_id,
                workflow_id: "workflow".parse().unwrap(),
            },
            signature: EnvelopeSignature::Secp256k1(signature),
        }
    }

    fn mock_signer() -> LocalSigner<SigningKey> {
        MnemonicBuilder::<English>::default()
            .word_count(24)
            .build_random()
            .unwrap()
    }

    fn mock_envelope(trigger_id: u64, data: impl Into<Bytes>) -> Envelope {
        // SimpleSubmit has its own data format, so we need to encode it
        let payload = DataWithId {
            triggerId: trigger_id,
            data: data.into(),
        };
        Envelope {
            payload: payload.abi_encode().into(),
            eventId: FixedBytes([0; 20]),
            ordering: FixedBytes([0; 12]),
        }
    }

    struct TestDeps {
        _anvil: AnvilInstance,
        _data_dir: TempDir,
        client: EvmSigningClient,
        state: HttpState,
        chain_name: ChainName,
    }

    impl TestDeps {
        async fn new() -> Self {
            let anvil = Anvil::new().spawn();
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

            let chain_name = ChainName::new("local").unwrap();

            config.chains.evm.insert(
                chain_name.clone(),
                EvmChainConfig {
                    chain_id: "31337".to_string(),
                    http_endpoint: Some(anvil.endpoint()),
                    ws_endpoint: Some(anvil.ws_endpoint()),
                    faucet_endpoint: None,
                    poll_interval_ms: None,
                },
            );

            config.credential =
                Some("test test test test test test test test test test test junk".to_string());

            let client_config = config
                .chains
                .evm
                .get(&chain_name)
                .unwrap()
                .signing_client_config(config.credential.clone().unwrap())
                .unwrap();

            let client = EvmSigningClient::new(client_config).await.unwrap();

            let state = HttpState::new(config).unwrap();

            Self {
                _anvil: anvil,
                _data_dir: data_dir,
                client,
                state,
                chain_name,
            }
        }

        pub async fn create_service(
            &self,
            service_id: ServiceID,
            service_manager_address: Address,
            service_handler_addresses: Vec<Address>,
        ) -> Service {
            let service = mock_service(
                self.chain_name.clone(),
                service_id,
                service_manager_address,
                service_handler_addresses,
            )
            .await;
            self.state.register_service(&service).unwrap();
            service
        }

        async fn deploy_simple_service_manager(&self) -> SimpleServiceManagerInstance<DynProvider> {
            SimpleServiceManager::deploy(self.client.provider.clone())
                .await
                .unwrap()
        }

        async fn deploy_simple_service_handler(
            &self,
            service_manager_address: Address,
        ) -> SimpleServiceHandlerInstance<DynProvider> {
            SimpleServiceHandler::deploy(self.client.provider.clone(), service_manager_address)
                .await
                .unwrap()
        }
    }
}
