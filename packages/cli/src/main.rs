use alloy_primitives::FixedBytes;
use alloy_provider::Provider;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::{ConfigExt, EvmChainConfigExt},
    evm_client::EvmSigningClient,
    service::fetch_service,
};
use wavs_cli::{
    args::Command,
    command::{
        deploy_service::{DeployService, DeployServiceArgs, SetServiceUrlArgs},
        exec_aggregator::{ExecAggregator, ExecAggregatorArgs},
        exec_component::{ExecComponent, ExecComponentArgs},
        service::handle_service_command,
        upload_component::{UploadComponent, UploadComponentArgs},
    },
    context::CliContext,
    util::{write_output_file, ComponentInput},
};
use wavs_types::SignatureKind;
use wavs_types::{ChainKeyId, Envelope, EnvelopeExt, IWavsServiceHandler};

// duplicated here instead of using the one in CliContext so
// that we don't end up accidentally using the CliContext one in e2e tests
pub(crate) async fn new_evm_client(
    ctx: &CliContext,
    chain_id: ChainKeyId,
) -> Result<EvmSigningClient> {
    let chain_config = ctx
        .config
        .chains
        .evm
        .get(&chain_id)
        .context(format!("chain id {chain_id} not found"))?
        .clone()
        .build(chain_id);

    let client_config = chain_config.signing_client_config(
        ctx.config
            .evm_credential
            .clone()
            .context("missing evm_credential")?,
    )?;

    let evm_client = EvmSigningClient::new(client_config).await?;

    Ok(evm_client)
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
            service_url,
            set_url,
            args: _,
        } => {
            let service = fetch_service(&service_url, &ctx.config.ipfs_gateway)
                .await
                .context(format!(
                    "Failed to fetch service from URL '{}' using gateway '{}'",
                    service_url, ctx.config.ipfs_gateway
                ))
                .unwrap();

            let set_service_url_args = if set_url {
                let provider = new_evm_client(&ctx, service.manager.chain().id.clone())
                    .await
                    .unwrap()
                    .provider;
                Some(SetServiceUrlArgs {
                    provider,
                    service_url,
                })
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
                match &res.wasm_response {
                    Some(wasm_response) => {
                        if let Err(e) = write_output_file(wasm_response, &path) {
                            eprintln!("Failed to write component output: {e}");
                            std::process::exit(1);
                        }
                    }
                    None => {
                        tracing::warn!(
                            "No output payload produced by component to save to {}",
                            path.display()
                        );
                    }
                }
            }

            // If submit_chain is provided, submit the result to the chain
            if let (Some(chain_key), Some(handler_address)) = (submit_chain, submit_handler) {
                if let Some(wasm_response) = &res.wasm_response {
                    tracing::info!(
                        "Submitting result to chain {} at address {}",
                        chain_key,
                        handler_address
                    );

                    // Create envelope from WASM response
                    let envelope = Envelope {
                        payload: wasm_response.payload.clone().into(),
                        eventId: FixedBytes::random(),
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

                    // Get EVM client for the chain
                    let evm_client = match new_evm_client(&ctx, chain_key.id).await {
                        Ok(client) => client,
                        Err(e) => {
                            eprintln!("Failed to create EVM client: {e}");
                            std::process::exit(1);
                        }
                    };

                    // Create signature using the EVM client's signer
                    let signature = envelope
                        .sign(
                            &evm_client.signer,
                            SignatureKind {
                                algorithm: wavs_types::SignatureAlgorithm::Secp256k1,
                                prefix: Some(wavs_types::SignaturePrefix::Eip191),
                            },
                        )
                        .await
                        .unwrap();

                    // Create contract instance
                    let contract =
                        IWavsServiceHandler::new(handler_address, evm_client.provider.clone());

                    // Get the latest block number for reference
                    let latest_block = match evm_client.provider.get_block_number().await {
                        Ok(block_num) => block_num,
                        Err(e) => {
                            eprintln!("Failed to get latest block number: {e}");
                            std::process::exit(1);
                        }
                    };

                    // Prepare signature data
                    let signature_data = IWavsServiceHandler::SignatureData {
                        signers: vec![evm_client.signer.address()],
                        signatures: vec![signature.data.into()],
                        referenceBlock: latest_block as u32,
                    };

                    // Convert to contract types
                    let contract_envelope = IWavsServiceHandler::Envelope {
                        eventId: envelope.eventId,
                        ordering: envelope.ordering,
                        payload: envelope.payload,
                    };

                    // Submit to chain
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
                } else {
                    tracing::warn!("No WASM response to submit to chain");
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
