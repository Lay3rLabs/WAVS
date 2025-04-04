use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::config::ConfigExt;
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
use wavs_types::ServiceManager;

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
            let res = DeployServiceRaw::run(&ctx, DeployServiceRawArgs { service })
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
        } => handle_service_command(&ctx, file, ctx.json, command)
            .await
            .unwrap(),
    }
}
