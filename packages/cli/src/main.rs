use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::config::ConfigExt;
use wavs_cli::{
    args::{CliTriggerKind, Command},
    command::{
        deploy_eigen_core::{DeployEigenCore, DeployEigenCoreArgs},
        deploy_eigen_service_manager::{DeployEigenServiceManager, DeployEigenServiceManagerArgs},
        deploy_service::{DeployService, DeployServiceArgs},
        deploy_service_raw::{DeployServiceRaw, DeployServiceRawArgs},
        exec_component::{ExecComponent, ExecComponentArgs},
        service::handle_service_command,
        upload_component::{UploadComponent, UploadComponentArgs},
    },
    context::CliContext,
    util::{read_component, ComponentInput},
};
use wavs_types::ComponentSource;

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

    let ctx = CliContext::try_new(&command, config, None).await.unwrap();

    match command {
        Command::DeployEigenCore {
            register_operator,
            chain,
            args: _,
        } => {
            let res = DeployEigenCore::run(
                &ctx,
                DeployEigenCoreArgs {
                    register_operator,
                    chain: chain.clone(),
                },
            )
            .await
            .unwrap();

            ctx.handle_deploy_result(res).unwrap();
        }
        Command::DeployService {
            component,
            trigger,
            trigger_chain,
            trigger_address,
            submit_address,
            submit,
            submit_chain,
            service_config,
            trigger_event_name,
            args: _,
        } => {
            let component = ComponentSource::Bytecode(read_component(&component).unwrap());

            let trigger = match (trigger, &trigger_address) {
                (Some(trigger), _) => trigger,
                (None, Some(trigger_address)) => {
                    if trigger_address.starts_with("0x") {
                        CliTriggerKind::EthContractEvent
                    } else {
                        CliTriggerKind::CosmosContractEvent
                    }
                }
                (None, None) => {
                    panic!("trigger is required to be set if trigger_address is not set");
                }
            };

            let res = DeployService::run(
                &ctx,
                DeployServiceArgs {
                    component,
                    trigger,
                    trigger_event_name,
                    trigger_chain,
                    trigger_address,
                    submit_address,
                    submit,
                    submit_chain,
                    service_config,
                },
            )
            .await
            .unwrap();

            if let Some(res) = res {
                ctx.handle_deploy_result(res).unwrap();
            }
        }
        Command::DeployServiceRaw { service, args: _ } => {
            let res = DeployServiceRaw::run(&ctx, DeployServiceRawArgs { service })
                .await
                .unwrap();

            ctx.handle_deploy_result(res).unwrap();
        }
        Command::DeployEigenServiceManager {
            chain,
            register_operator,
            args: _,
        } => {
            let res = DeployEigenServiceManager::run(
                &ctx,
                DeployEigenServiceManagerArgs {
                    chain: chain.clone(),
                    register_operator,
                },
            )
            .await
            .unwrap();

            ctx.handle_deploy_result(res).unwrap();
        }
        Command::UploadComponent { component, args: _ } => {
            let res = UploadComponent::run(
                &ctx.config,
                UploadComponentArgs {
                    component_path: component,
                },
            )
            .await
            .unwrap();

            ctx.handle_display_result(res);
        }
        Command::Exec {
            component,
            input,
            service_config,
            fuel_limit,
            args: _,
        } => {
            let res = ExecComponent::run(
                &ctx.config,
                ExecComponentArgs {
                    component_path: component,
                    service_config,
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
        } => handle_service_command(&ctx, file, command).await.unwrap(),
    }
}
