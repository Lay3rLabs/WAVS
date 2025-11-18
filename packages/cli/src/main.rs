use alloy_primitives::FixedBytes;
use alloy_provider::Provider;
use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use layer_climb::prelude::cosmos_hub_derivation;
use layer_climb::prelude::KeySigner;
use layer_climb::signing::SigningClient;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigExt, EvmChainConfigExt},
    evm_client::EvmSigningClient,
    service::fetch_service,
};
use wavs_cli::{
    args::Command,
    command::{
        deploy_service::{DeployService, DeployServiceArgs, SetServiceUriArgs},
        exec_aggregator::{ExecAggregator, ExecAggregatorArgs},
        exec_component::{ExecComponent, ExecComponentArgs},
        service::handle_service_command,
        upload_component::{UploadComponent, UploadComponentArgs},
    },
    context::CliContext,
    util::{write_output_file, ComponentInput},
};
use wavs_types::EnvelopeSigner;
use wavs_types::SignatureKind;
use wavs_types::{ChainKeyId, Envelope, IWavsServiceHandler};

// Shared function to create EVM client with any credential
// duplicated here instead of using the one in CliContext so
// that we don't end up accidentally using the CliContext one in e2e tests
async fn new_evm_client_with_credential(
    ctx: &CliContext,
    chain_id: ChainKeyId,
    credential: &wavs_types::Credential,
    hd_index: Option<u32>,
) -> Result<EvmSigningClient> {
    let chain_config = ctx
        .config
        .chains
        .read()
        .map_err(|_| anyhow!("Chains lock is poisoned"))?
        .evm
        .get(&chain_id)
        .context(format!("chain id {chain_id} not found"))?
        .clone()
        .build(chain_id);

    let mut client_config = chain_config.signing_client_config(credential.clone())?;
    if let Some(hd_index) = hd_index {
        client_config.hd_index = Some(hd_index);
    }

    let evm_client = EvmSigningClient::new(client_config).await?;

    Ok(evm_client)
}

async fn new_cosmos_client_with_credential(
    ctx: &CliContext,
    chain_id: ChainKeyId,
    credential: &wavs_types::Credential,
    hd_index: Option<u32>,
) -> Result<SigningClient> {
    let chain_config = ctx
        .config
        .chains
        .read()
        .map_err(|_| anyhow!("Chains lock is poisoned"))?
        .cosmos
        .get(&chain_id)
        .context(format!("chain id {chain_id} not found"))?
        .clone()
        .build(chain_id);

    let derivation = match hd_index {
        Some(hd_index) => Some(cosmos_hub_derivation(hd_index)?),
        None => None,
    };
    let signer = KeySigner::new_mnemonic_str(credential, derivation.as_ref())?;

    let cosmos_client = SigningClient::new(chain_config.into(), signer, None).await?;

    Ok(cosmos_client)
}

pub(crate) async fn new_evm_client(
    ctx: &CliContext,
    chain_id: ChainKeyId,
) -> Result<EvmSigningClient> {
    new_evm_client_with_credential(
        ctx,
        chain_id,
        &ctx.config
            .evm_credential
            .clone()
            .context("missing evm_credential")?,
        None,
    )
    .await
}

pub(crate) async fn new_cosmos_client(
    ctx: &CliContext,
    chain_id: ChainKeyId,
) -> Result<SigningClient> {
    new_cosmos_client_with_credential(
        ctx,
        chain_id,
        &ctx.config
            .evm_credential
            .clone()
            .context("missing evm_credential")?,
        None,
    )
    .await
}

#[tokio::main]
async fn main() {
    let command = Command::parse();
    let config = command.config();

    // setup tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_target(false),
        )
        .with(config.tracing_env_filter().unwrap())
        .try_init()
        .unwrap();

    let ctx = CliContext::try_new(&command, config.clone(), None)
        .await
        .unwrap();

    match command {
        Command::DeployService {
            service_uri,
            set_uri,
            args: _,
        } => {
            let service = fetch_service(&service_uri, &ctx.config.ipfs_gateway)
                .await
                .context(format!(
                    "Failed to fetch service from URL '{}' using gateway '{}'",
                    service_uri, ctx.config.ipfs_gateway
                ))
                .unwrap();

            let set_service_url_args = if set_uri {
                match service.manager {
                    wavs_types::ServiceManager::Evm { ref chain, .. } => {
                        let provider = new_evm_client(&ctx, chain.id.clone())
                            .await
                            .unwrap()
                            .provider;
                        Some(SetServiceUriArgs::new_evm(provider, service_uri.clone()))
                    }
                    wavs_types::ServiceManager::Cosmos { ref chain, .. } => {
                        let client = new_cosmos_client(&ctx, chain.id.clone()).await.unwrap();
                        Some(SetServiceUriArgs::new_cosmos(client, service_uri.clone()))
                    }
                }
            } else {
                None
            };

            let res = DeployService::run(
                &ctx,
                DeployServiceArgs {
                    service_manager: service.manager.clone(),
                    set_service_url_args,
                },
            )
            .await
            .unwrap();

            ctx.handle_deploy_result(res).unwrap();
        }
        Command::UploadComponent {
            component_path,
            args: _,
        } => {
            let res = UploadComponent::run(&ctx.config, UploadComponentArgs { component_path })
                .await
                .unwrap();

            ctx.handle_display_result(res);
        }
        Command::Exec {
            component,
            input,
            fuel_limit,
            time_limit,
            config,
            output_file,
            submit_chain,
            submit_handler,
            simulates_trigger,
            operator_credential,
            operator_hd_index,
            args: _,
        } => {
            let config = config
                .into_iter()
                .filter_map(|pair| {
                    if let Some((key, value)) = pair.split_once('=') {
                        Some((key.to_string(), value.to_string()))
                    } else {
                        None
                    }
                })
                .collect();

            let res = match ExecComponent::run(
                &ctx.config,
                ExecComponentArgs {
                    component_path: component,
                    input: ComponentInput::new(input),
                    time_limit,
                    fuel_limit,
                    config,
                    simulates_trigger,
                },
            )
            .await
            {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Failed to execute component: {e}");
                    std::process::exit(1);
                }
            };

            // If an output file was requested, write the wasm response as JSON
            if let Some(path) = output_file {
                if res.wasm_responses.is_empty() {
                    tracing::warn!(
                        "No output payload produced by component to save to {}",
                        path.display()
                    );
                } else if let Err(e) = write_output_file(&res.wasm_responses, &path) {
                    eprintln!("Failed to write component output: {e}");
                    std::process::exit(1);
                }
            }

            // If submit_chain is provided, submit the result to the chain
            if let (Some(chain_key), Some(handler_address), Some(operator_credential)) =
                (submit_chain, submit_handler, operator_credential)
            {
                if res.wasm_responses.is_empty() {
                    tracing::warn!("No WASM response to submit to chain");
                } else {
                    for wasm_response in &res.wasm_responses {
                        tracing::info!(
                            "Submitting result to chain {} at address {}",
                            chain_key,
                            handler_address
                        );

                        // Create envelope from WASM response
                        let envelope = Envelope {
                            payload: wasm_response.payload.clone().into(),
                            eventId: FixedBytes::new(rand::random()),
                            ordering: match wasm_response.ordering {
                                Some(ordering) => {
                                    // Convert u64 ordering to 12-byte FixedBytes by placing the u64 in the first 8 bytes
                                    // This preserves the ordering value while meeting the FixedBytes<12> requirement
                                    let mut bytes = [0u8; 12];
                                    bytes[..8].copy_from_slice(&ordering.to_le_bytes());
                                    FixedBytes(bytes)
                                }
                                None => FixedBytes::default(),
                            },
                        };

                        // Get EVM client for the chain (for transaction submission)
                        let evm_client = match new_evm_client(&ctx, chain_key.id.clone()).await {
                            Ok(client) => client,
                            Err(e) => {
                                eprintln!("Failed to create EVM client: {e}");
                                std::process::exit(1);
                            }
                        };

                        // Get operator EVM client for envelope signing
                        let operator_evm_client = match new_evm_client_with_credential(
                            &ctx,
                            chain_key.id.clone(),
                            &operator_credential,
                            operator_hd_index,
                        )
                        .await
                        {
                            Ok(client) => client,
                            Err(e) => {
                                eprintln!("Failed to create operator EVM client: {e}");
                                std::process::exit(1);
                            }
                        };

                        // Create signature using the operator EVM client's signer
                        let signature = envelope
                            .sign(&operator_evm_client.signer, SignatureKind::evm_default())
                            .await
                            .unwrap();

                        // Create contract instance
                        let contract =
                            IWavsServiceHandler::new(handler_address, evm_client.provider.clone());

                        // Get the block number just before the latest block for reference
                        let previous_block = match evm_client.provider.get_block_number().await {
                            Ok(block_num) => block_num - 1,
                            Err(e) => {
                                eprintln!("Failed to get latest block number: {e}");
                                std::process::exit(1);
                            }
                        };

                        // Prepare signature data
                        let signature_data =
                            match envelope.signature_data(vec![signature], previous_block) {
                                Ok(data) => data,
                                Err(e) => {
                                    eprintln!("Failed to prepare signature data: {e}");
                                    std::process::exit(1);
                                }
                            };

                        // Convert to contract types
                        let contract_envelope = IWavsServiceHandler::Envelope {
                            eventId: envelope.eventId,
                            ordering: envelope.ordering,
                            payload: envelope.payload,
                        };

                        // Submit to chain using the original EVM client (as transaction sender)
                        match contract
                            .handleSignedEnvelope(contract_envelope, signature_data)
                            .send()
                            .await
                        {
                            Ok(tx) => {
                                tracing::info!("Transaction submitted: {:?}", tx.tx_hash());
                            }
                            Err(e) => {
                                eprintln!("Failed to submit to chain: {e}");
                                std::process::exit(1);
                            }
                        }
                    }
                }
            }

            ctx.handle_display_result(res);
        }
        Command::Service {
            command,
            file,
            args: _,
        } => handle_service_command(&ctx, file, ctx.json, command)
            .await
            .unwrap(),
        Command::ExecAggregator {
            component,
            packet,
            fuel_limit,
            time_limit,
            config,
            output_file,
            args: _,
        } => {
            let config = config
                .unwrap_or_default()
                .into_iter()
                .filter_map(|pair| {
                    if let Some((key, value)) = pair.split_once('=') {
                        Some((key.to_string(), value.to_string()))
                    } else {
                        None
                    }
                })
                .collect();

            let res = match ExecAggregator::run(
                &ctx.config,
                ExecAggregatorArgs {
                    component,
                    packet,
                    fuel_limit,
                    time_limit,
                    config,
                },
            )
            .await
            {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Failed to execute aggregator: {e}");
                    std::process::exit(1);
                }
            };

            // If an output file was requested, write the aggregator result as JSON
            if let Some(path) = output_file {
                if let Err(e) = write_output_file(&res, &path) {
                    eprintln!("Failed to write aggregator output: {e}");
                    std::process::exit(1);
                }
            }

            ctx.handle_display_result(res);
        }
    }
}
