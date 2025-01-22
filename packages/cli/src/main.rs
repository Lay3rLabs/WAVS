use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{avs_client::SignedData, config::ConfigExt};
use wavs_cli::{
    args::Command,
    command::{
        add_task::{AddTask, AddTaskArgs},
        deploy_eigen_core::{DeployEigenCore, DeployEigenCoreArgs},
        deploy_service::{ComponentSource, DeployService, DeployServiceArgs},
        exec_component::{ExecComponent, ExecComponentArgs},
    },
    context::CliContext,
    display::DisplayBuilder,
    util::ComponentInput,
};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

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

    let mut display = DisplayBuilder::new();

    let mut ctx = CliContext::try_new(&command, config, None).await.unwrap();

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

            let DeployEigenCore { addresses } = res;

            display.core_contracts = Some(addresses);

            ctx.save_deployment().unwrap();
        }
        Command::DeployService {
            register_operator,
            component,
            trigger,
            trigger_chain,
            trigger_address,
            cosmos_trigger_code_id,
            submit,
            submit_chain,
            service_config,
            trigger_event_name,
            args: _,
        } => {
            let res = DeployService::run(
                &ctx,
                DeployServiceArgs {
                    register_operator,
                    component: ComponentSource::Path(component),
                    trigger,
                    trigger_event_name,
                    trigger_chain,
                    trigger_address,
                    cosmos_trigger_code_id,
                    submit,
                    submit_chain,
                    service_config,
                },
            )
            .await
            .unwrap();

            if let Some(DeployService {
                service_id,
                workflows,
            }) = res
            {
                ctx.save_deployment().unwrap();

                display.service = Some((service_id, workflows));
            }
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

            if let Some(signed_data) = res.and_then(|res| res.signed_data) {
                display.signed_data = Some(signed_data);
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

            let ExecComponent {
                output_bytes,
                gas_used,
            } = res;

            display.signed_data = Some(SignedData {
                data: output_bytes,
                signature: vec![],
            });

            display.gas_used = Some(gas_used);
        }
    }

    display.show().unwrap();
}
