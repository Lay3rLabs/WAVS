use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    config::ConfigExt,
    eth_client::{EthClientBuilder, EthSigningClient},
};
use wavs_cli::{
    args::Command,
    command::{
        deploy_service_raw::{DeployServiceRaw, DeployServiceRawArgs},
        exec_component::{ExecComponent, ExecComponentArgs},
        service::handle_service_command,
        upload_component::{UploadComponent, UploadComponentArgs},
    },
    context::CliContext,
    util::ComponentInput,
};
use wavs_types::ChainName;

pub(crate) async fn new_eth_client(
    ctx: &CliContext,
    chain_name: &ChainName,
) -> Result<EthSigningClient> {
    let chain_config = ctx
        .config
        .chains
        .eth
        .get(chain_name)
        .context(format!("chain {chain_name} not found"))?
        .clone();

    let client_config = chain_config.to_client_config(None, ctx.config.eth_mnemonic.clone(), None);

    let eth_client = EthClientBuilder::new(client_config).build_signing().await?;

    Ok(eth_client)
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
        Command::DeployServiceRaw { service, args: _ } => {
            let provider = new_eth_client(&ctx, service.manager.chain_name())
                .await
                .unwrap()
                .provider;

            let res = DeployServiceRaw::run(&ctx, provider, DeployServiceRawArgs { service })
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
            args: _,
        } => {
            let res = ExecComponent::run(
                &ctx.config,
                ExecComponentArgs {
                    component_path: component,
                    input: ComponentInput::new(input),
                    fuel_limit,
                },
            )
            .await
            .unwrap();

            ctx.handle_display_result(res);
        }
        Command::Service {
            command,
            file,
            args: _,
        } => handle_service_command(&ctx, file, ctx.json, command)
            .await
            .unwrap(),
    }
}
