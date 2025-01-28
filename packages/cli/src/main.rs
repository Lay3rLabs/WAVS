use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{config::ConfigExt, types::ComponentSource};
use wavs_cli::{
    args::Command,
    command::{
        add_task::{AddTask, AddTaskArgs},
        deploy_eigen_core::{DeployEigenCore, DeployEigenCoreArgs},
        deploy_eigen_service_manager::{DeployEigenServiceManager, DeployEigenServiceManagerArgs},
        deploy_service::{DeployService, DeployServiceArgs},
        exec_component::{ExecComponent, ExecComponentArgs},
    },
    context::CliContext,
    util::{read_component, ComponentInput},
};

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
            register_operator,
            component,
            trigger,
            trigger_chain,
            trigger_address,
            submit_address,
            cosmos_trigger_code_id,
            submit,
            submit_chain,
            service_config,
            trigger_event_name,
            args: _,
        } => {
            let component = ComponentSource::Bytecode(read_component(component).unwrap());

            let res = DeployService::run(
                &ctx,
                DeployServiceArgs {
                    register_operator,
                    component,
                    trigger,
                    trigger_event_name,
                    trigger_chain,
                    trigger_address,
                    submit_address,
                    cosmos_trigger_code_id,
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
        Command::DeployEigenServiceManager {
            chain,
            service_handler,
            register_operator,
            args: _,
        } => {
            let res = DeployEigenServiceManager::run(
                &ctx,
                DeployEigenServiceManagerArgs {
                    chain: chain.clone(),
                    service_handler: service_handler.parse().unwrap(),
                    register_operator,
                },
            )
            .await
            .unwrap();

            ctx.handle_deploy_result(res).unwrap();
        }
        Command::AddTask {
            service_id,
            workflow_id,
            input,
            result_timeout_ms,
            args: _,
        } => {
            let res = AddTask::run(
                &ctx,
                AddTaskArgs {
                    service_id,
                    workflow_id,
                    input: ComponentInput::Stdin(input),
                    result_timeout: if result_timeout_ms > 0 {
                        Some(std::time::Duration::from_millis(result_timeout_ms))
                    } else {
                        None
                    },
                },
            )
            .await
            .unwrap();

            if let Some(res) = res {
                ctx.handle_display_result(res);
            }
        }
        Command::Exec {
            component,
            input,
            args: _,
        } => {
            let res = ExecComponent::run(ExecComponentArgs {
                component,
                input: ComponentInput::Stdin(input),
            })
            .await
            .unwrap();

            ctx.handle_display_result(res);
        }
    }
}
