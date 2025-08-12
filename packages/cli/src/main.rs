use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::service::fetch_service;
use utils::{
    config::{ConfigExt, EvmChainConfigExt},
    evm_client::EvmSigningClient,
};
use wavs_cli::command::deploy_service::SetServiceUrlArgs;
use wavs_cli::{
    args::Command,
    command::{
        deploy_service::{DeployService, DeployServiceArgs},
        exec_aggregator::{ExecAggregator, ExecAggregatorArgs},
        exec_component::{ExecComponent, ExecComponentArgs},
        service::handle_service_command,
        upload_component::{UploadComponent, UploadComponentArgs},
    },
    context::CliContext,
    util::ComponentInput,
};
use wavs_types::ChainName;

// duplicated here instead of using the one in CliContext so
// that we don't end up accidentally using the CliContext one in e2e tests
pub(crate) async fn new_evm_client(
    ctx: &CliContext,
    chain_name: &ChainName,
) -> Result<EvmSigningClient> {
    let chain_config = ctx
        .config
        .chains
        .evm
        .get(chain_name)
        .context(format!("chain {chain_name} not found"))?
        .clone();

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
                let provider = new_evm_client(&ctx, service.manager.chain_name())
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
            args: _,
        } => {
            let config = config
                .into_iter()
                .filter_map(|pair| {
                    if let Some((key, value)) = pair.split_once('=') {
                        Some((key.to_string(), value.to_string()))
                    } else {
                        None // skip malformed entries
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
            args: _,
        } => {
            // Process config similar to exec command
            let config = config
                .unwrap_or_default()
                .into_iter()
                .filter_map(|pair| {
                    if let Some((key, value)) = pair.split_once('=') {
                        Some((key.to_string(), value.to_string()))
                    } else {
                        None // skip malformed entries
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

            ctx.handle_display_result(res);
        }
    }
}
