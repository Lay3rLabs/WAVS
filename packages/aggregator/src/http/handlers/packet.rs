use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use anyhow::{anyhow, bail, ensure};
use axum::{extract::State, response::IntoResponse, Json};
use wavs_types::{
    aggregator::{AddPacketRequest, AddPacketResponse},
    Aggregator, EvmContractSubmission, IWavsServiceManager, Packet,
};

use crate::http::{
    error::AnyError,
    state::{HttpState, PacketQueue, QueuedPacket},
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
) -> anyhow::Result<Vec<AddPacketResponse>> {
    let event_id = packet.event_id();

    let route = packet.route.clone();

    let service = state.get_service(&packet.route)?;
    let aggregators = &service.workflows[&packet.route.workflow_id].aggregators;

    if aggregators.is_empty() {
        bail!(
            "No aggregator configured for workflow {} on service {}",
            route.workflow_id,
            route.service_id
        );
    }

    state.event_transaction.run(event_id.clone(), {
        let state = state.clone();
        let envelope = packet.envelope.clone();
        || async move {
            let mut queue = state.get_live_packet_queue(&event_id)?;
            let mut responses: Vec<AddPacketResponse> = Vec::new();

            for (index, aggregator) in aggregators.iter().enumerate() {
                match aggregator {
                    Aggregator::Evm(EvmContractSubmission {
                        chain_name,
                        address,
                        max_gas,
                    }) => {
                        // this implicitly validates that the signature is valid
                        let signer = packet.signature.evm_signer_address(&packet.envelope)?;

                        let client = state.get_evm_client(chain_name).await?;
                        let service_manager = IWavsServiceManager::new(
                            service.manager.evm_address_unchecked(),
                            client.provider.clone(),
                        );
                        // Get the threshold
                        let threshold = service_manager
                            .getLastCheckpointThresholdWeight()
                            .call()
                            .await?;

                        let weight = service_manager.getOperatorWeight(signer).call().await?;
                        let mut total_weight = weight;

                        // Sum up weights
                        for signer in queue.iter().map(|queued| queued.signer)
                        {
                            // TODO, contract should have a method to get multiple weights in one call
                            // but it doesn't really matter until those weights can change under our feet
                            let weight = service_manager.getOperatorWeight(signer).call().await?;
                            total_weight = weight
                                .checked_add(total_weight)
                                .ok_or(anyhow!("Total weight calculation overflowed"))?;
                        }

                        // get the current packets again, in case it's changed since last await point
                        validate_packet(packet, &queue, signer, weight)?;

                        if index == 0 {
                            // update the saved queue, but only for first aggregator
                            // invariant: they should all see the same queue
                            queue.push(QueuedPacket {
                                packet: packet.clone(),
                                signer,
                            });

                            state
                                .save_packet_queue(&event_id, PacketQueue::Alive(queue.clone()))?;
                        }

                        // TODO:
                        // given the total power of the quorum (which could be, say, 60% of the total operator set power)
                        // we need to calculate the power of the signers so far, and see if it meets the quorum power
                        // we don't care about count, we care about the power of the signers
                        // right now this is just hardcoded for demo purposes
                        if total_weight >= threshold {
                            if threshold.is_zero() {
                                tracing::warn!(
                                    "you are using threshold of 0 in your AVS quorum, best to only do this for testing"
                                );
                            }

                            let signatures = queue
                                .iter()
                                .map(|queued| queued.packet.signature.clone())
                                .collect();

                            let block_height = client.provider.get_block_number().await?;

                            let tx_receipt = client
                                .send_envelope_signatures(
                                    envelope.clone(),
                                    signatures,
                                    block_height,
                                    *address,
                                    *max_gas,
                                )
                                .await?;

                            responses.push(AddPacketResponse::Sent {
                                tx_receipt: Box::new(tx_receipt),
                                count: queue.len()
                            });
                        } else {
                            responses.push(AddPacketResponse::Aggregated {
                                count: queue.len(),
                            });
                        }
                    }
                }
            }

            if responses.len() != aggregators.len() {
                bail!("Unexpected number of responses: expected {}, got {}", aggregators.len(), responses.len());
            }

            if responses.iter().all(|response| matches!(response, AddPacketResponse::Sent { .. })) {
                // all aggregator destinations reached quorum and had their packets sent, burn the event
                state.save_packet_queue(&event_id, PacketQueue::Burned)?;
            } else {
                let mut sent_count = 0;
                let mut aggregated_count = 0;
                for response in responses.iter() {
                    match response {
                        AddPacketResponse::Sent { .. } => sent_count += 1,
                        AddPacketResponse::Aggregated { count } => aggregated_count += count,
                    }
                }
                // some aggregator destinations 
                tracing::warn!("Mixed responses: {} destinations sent, {} destinations aggregated", sent_count, aggregated_count);
            }

            Ok(responses)
        }
    }).await
}

fn validate_packet(
    packet: &Packet,
    queue: &[QueuedPacket],
    signer: Address,
    operator_weight: U256,
) -> anyhow::Result<()> {
    match queue.first() {
        None => {}
        Some(prev) => {
            // check if the packet is the same as the last one
            if packet.envelope != prev.packet.envelope {
                bail!("Unexpected envelope difference!");
            }

            // see https://github.com/Lay3rLabs/wavs-middleware/issues/54
            // if packet.block_height != last_packet.block_height {
            //     bail!("Unexpected block height difference!");
            // }
        }
    }

    for queued_packet in queue {
        if signer == queued_packet.signer {
            bail!("Signer {} already in queue", signer);
        }
    }

    ensure!(!operator_weight.is_zero(), "Operator is not registered");

    // TODO: ensure that the signer is in the operator set

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{args::CliArgs, config::Config};
    use alloy_node_bindings::{Anvil, AnvilInstance};
    use alloy_primitives::{Bytes, FixedBytes};
    use alloy_provider::DynProvider;
    use alloy_signer::{k256::ecdsa::SigningKey, SignerSync};
    use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
    use alloy_sol_types::sol;
    use futures::{stream::FuturesUnordered, StreamExt};
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
    use TestServiceManager::TestServiceManagerInstance;

    #[test]
    fn packet_validation() {
        let mut queue = Vec::new();

        let signer_1 = mock_signer();
        let signer_2 = mock_signer();
        let envelope_1 = mock_envelope([1, 2, 3]);
        let envelope_2 = mock_envelope([4, 5, 6]);

        let packet = mock_packet(&signer_1, &envelope_1, "service-1".parse().unwrap());

        let derived_signer_1_address = packet
            .signature
            .evm_signer_address(&packet.envelope)
            .unwrap();
        assert_eq!(derived_signer_1_address, signer_1.address());

        // empty queue is okay
        validate_packet(&packet, &queue, signer_1.address(), U256::ONE).unwrap();

        queue.push(QueuedPacket {
            packet: packet.clone(),
            signer: signer_1.address(),
        });

        // "fails" (expectedly) because the signer is the same
        let packet = mock_packet(&signer_1, &envelope_1, "service-1".parse().unwrap());
        validate_packet(&packet, &queue, signer_1.address(), U256::ONE).unwrap_err();

        // "fails" (expectedly) because the envelope is different
        let packet = mock_packet(&signer_2, &envelope_2, "service-1".parse().unwrap());
        validate_packet(&packet, &queue, signer_2.address(), U256::ONE).unwrap_err();

        // "fails" (expectedly) because the operator is not registered (0 weight)
        let packet = mock_packet(&signer_2, &envelope_1, "service-1".parse().unwrap());
        validate_packet(&packet, &queue, signer_2.address(), U256::ZERO).unwrap_err();

        // passes because the signer is different but envelope is the same
        validate_packet(&packet, &queue, signer_2.address(), U256::ONE).unwrap();
        queue.push(QueuedPacket {
            packet: packet.clone(),
            signer: signer_2.address(),
        });
    }

    #[tokio::test]
    async fn process_many_packets_serial() {
        process_many_packets(false).await;
    }

    #[tokio::test]
    async fn process_many_packets_concurrent() {
        process_many_packets(true).await;
    }

    async fn process_many_packets(concurrent: bool) {
        let deps = TestDeps::new().await;

        let service_manager = deps.deploy_simple_service_manager().await;
        let service = deps
            .create_service("service-2".parse().unwrap(), *service_manager.address())
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

        for _ in 0..NUM_THRESHOLD {
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

        let envelope = mock_envelope([1, 2, 3]);

        let seen_count: Arc<Mutex<HashSet<usize>>> = Arc::new(Mutex::new(HashSet::new()));

        if !concurrent {
            for signer in signers {
                let packet = mock_packet(&signer, &envelope, service.id.clone());
                if let AddPacketResponse::Aggregated { count } =
                    process_packet(deps.state.clone(), &packet)
                        .await
                        .unwrap()
                        .pop()
                        .unwrap()
                {
                    let mut seen_count = seen_count.lock().unwrap();
                    if !seen_count.insert(count) {
                        panic!("Duplicate count: {}", count);
                    }
                }
            }
        } else {
            let mut futures = FuturesUnordered::new();
            for signer in signers {
                let packet = mock_packet(&signer, &envelope, service.id.clone());
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
    }

    fn mock_service(
        chain_name: ChainName,
        service_id: ServiceID,
        service_manager_address: Address,
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
                aggregators: vec![wavs_types::Aggregator::Evm(
                    wavs_types::EvmContractSubmission {
                        chain_name: chain_name.clone(),
                        address: FixedBytes([2; 20]).into(),
                        max_gas: None,
                    },
                )],
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

    fn mock_envelope(payload: impl Into<Bytes>) -> Envelope {
        Envelope {
            payload: payload.into(),
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
                    aggregator_endpoint: None,
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
        ) -> Service {
            let service =
                mock_service(self.chain_name.clone(), service_id, service_manager_address);
            self.state.register_service(&service).unwrap();
            service
        }

        async fn deploy_simple_service_manager(&self) -> TestServiceManagerInstance<DynProvider> {
            TestServiceManager::deploy(self.client.provider.clone())
                .await
                .unwrap()
        }
    }

    sol!(
        // solc TestServiceManager.sol --via-ir --optimize --bin
        #[sol(rpc, bytecode="608080604052346015576104b5908161001a8239f35b5f80fdfe60806040526004361015610011575f80fd5b5f3560e01c806308fc760a146103f85780630e6b1110146103c3578063314f3a49146103a65780635f11301b1461023c57806398ec1ac914610204578063b933fa74146101e7578063cc922c6a146100ee578063cd71589e146100995763fb8524b11461007c575f80fd5b3461009557602036600319011261009557600435600255005b5f80fd5b346100955760403660031901126100955760043567ffffffffffffffff81116100955760609060031990360301126100955760243567ffffffffffffffff811161009557606090600319903603011261009557005b34610095575f366003190112610095576040515f5f549061010e82610427565b8084526020840192600181169081156101ce575060011461018d575b50829003601f01601f191682019167ffffffffffffffff831181841017610179576040918391828452602083525180918160208501528484015e5f828201840152601f01601f19168101030190f35b634e487b7160e01b5f52604160045260245ffd5b90505f80525f5160206104605f395f51905f525f905b8282106101b85750602091508301018361012a565b60018160209254838589010152019101906101a3565b60ff1916845250151560051b830160200190508361012a565b34610095575f366003190112610095576020600254604051908152f35b34610095576020366003190112610095576001600160a01b03610225610411565b165f526001602052602060405f2054604051908152f35b346100955760203660031901126100955760043567ffffffffffffffff8111610095573660238201121561009557806004013567ffffffffffffffff8111610095573660248284010111610095576102945f54610427565b601f8111610342575b505f601f82116001146102d85781925f926102ca575b50505f19600383901b1c191660019190911b175f55005b6024925001013582806102b3565b601f198216925f5160206104605f395f51905f52915f5b8581106103275750836001951061030b575b505050811b015f55005b01602401355f19600384901b60f8161c19169055828080610301565b909260206001819260248787010135815501940191016102ef565b601f820160051c5f5160206104605f395f51905f52019060208310610391575b601f0160051c5f5160206104605f395f51905f5201905b818110610386575061029d565b5f8155600101610379565b5f5160206104605f395f51905f529150610362565b34610095575f366003190112610095576020600354604051908152f35b34610095576040366003190112610095576001600160a01b036103e4610411565b165f52600160205260243560405f20555f80f35b3461009557602036600319011261009557600435600355005b600435906001600160a01b038216820361009557565b90600182811c92168015610455575b602083101461044157565b634e487b7160e01b5f52602260045260245ffd5b91607f169161043656fe290decd9548b62a8d60345a988386fc84ba6bc95484008f6362f93160ef3e563a264697066735822122019df7c28a706099281ade4f02b0f476af1f8afd081e40b2359de39cb25c7735664736f6c634300081d0033")]
        contract TestServiceManager {
            string private serviceURI;

            mapping(address => uint256) private operatorWeights;
            uint256 private lastCheckpointThresholdWeight;
            uint256 private lastCheckpointTotalWeight;


            struct SignatureData {
                address[] operators;
                bytes[] signatures;
                uint32 referenceBlock;
            }
            struct Envelope {
                bytes20 eventId;
                // currently unused, for future version. added now for padding
                bytes12 ordering;
                bytes payload;
            }

            function validate(
                Envelope calldata envelope,
                SignatureData calldata signatureData
            ) external view {
                // always valid
            }

            function getServiceURI() external view returns (string memory) {
                return serviceURI;
            }

            function setServiceURI(string calldata _serviceURI) external {
                serviceURI = _serviceURI;
            }

            function setOperatorWeight(address operator, uint256 weight) external {
                operatorWeights[operator] = weight;
            }

            function setLastCheckpointThresholdWeight(uint256 weight) external {
                lastCheckpointThresholdWeight = weight;
            }

            function setLastCheckpointTotalWeight(uint256 weight) external {
                lastCheckpointTotalWeight = weight;
            }

            function getOperatorWeight(address operator) external view returns (uint256) {
                return operatorWeights[operator];
            }

            function getLastCheckpointThresholdWeight() external view returns (uint256) {
                return lastCheckpointThresholdWeight;
            }

            function getLastCheckpointTotalWeight() external view returns (uint256) {
                return lastCheckpointTotalWeight;
            }
        }
    );
}
