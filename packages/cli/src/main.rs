mod args;
mod client;
mod context;
mod display;
mod task;

use args::Command;
use clap::Parser;
use client::{get_avs_client, get_eigen_client, HttpClient};
use context::WavsContext;
use display::{
    display_core_contracts, display_eth_trigger_echo_digest, display_eth_trigger_echo_service_id,
    display_layer_service_contracts, display_response_signature,
};
use rand::{
    distributions::{Alphanumeric, DistString},
    rngs::OsRng,
};
use task::run_eth_trigger_echo_task;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wavs::apis::{ServiceID, WorkflowID};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    // setup tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_target(false),
        )
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .unwrap();

    match Command::parse() {
        Command::DeployCore {
            register_operator,
            wavs,
        } => {
            let ctx = WavsContext::new(wavs).unwrap();
            let eigen_client = get_eigen_client(ctx).await;
            let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

            if register_operator {
                eigen_client
                    .register_operator(&core_contracts)
                    .await
                    .unwrap();
            }

            display_core_contracts(&core_contracts);
        }

        Command::DeployService {
            add_service,
            wavs,
            core_contracts,
            register_operator,
            digests,
        } => {
            let ctx = WavsContext::new(wavs).unwrap();
            let core_contracts = core_contracts.into();

            let eigen_client = get_eigen_client(ctx.clone()).await;
            let avs_client = get_avs_client(&eigen_client, core_contracts).await;

            if register_operator {
                avs_client.register_operator(&mut OsRng).await.unwrap();
            }

            if add_service {
                let http_client = HttpClient::new(ctx);

                let digest = match digests.digest_hello_world {
                    None => {
                        let digest = http_client.upload_eth_trigger_echo_digest().await;
                        display_eth_trigger_echo_digest(&digest);
                        digest
                    }
                    Some(digest) => digest,
                };

                let service_id = http_client
                    .create_eth_trigger_echo_service(
                        avs_client.layer.trigger,
                        avs_client.layer.service_manager,
                        digest,
                    )
                    .await;
                display_eth_trigger_echo_service_id(&service_id);
            }

            display_layer_service_contracts(&avs_client.layer);
        }

        Command::DeployAll {
            add_service,
            wavs,
            register_core_operator,
            register_service_operator,
            digests,
        } => {
            let ctx = WavsContext::new(wavs).unwrap();
            let eigen_client = get_eigen_client(ctx.clone()).await;
            let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

            if register_core_operator {
                eigen_client
                    .register_operator(&core_contracts)
                    .await
                    .unwrap();
            }

            let avs_client = get_avs_client(&eigen_client, core_contracts.clone()).await;

            if register_service_operator {
                avs_client.register_operator(&mut OsRng).await.unwrap();
            }

            if add_service {
                let http_client = HttpClient::new(ctx);

                let digest = match digests.digest_hello_world {
                    None => {
                        let digest = http_client.upload_eth_trigger_echo_digest().await;
                        display_eth_trigger_echo_digest(&digest);
                        digest
                    }
                    Some(digest) => digest,
                };

                let service_id = http_client
                    .create_eth_trigger_echo_service(
                        avs_client.layer.trigger,
                        avs_client.layer.service_manager,
                        digest,
                    )
                    .await;
                display_eth_trigger_echo_service_id(&service_id);
            }

            display_core_contracts(&core_contracts);
            display_layer_service_contracts(&avs_client.layer);
        }

        Command::AddTask {
            watch_wavs,
            wavs,
            trigger_addr,
            service_manager_addr,
            service_id,
            workflow_id,
            name,
        } => {
            let ctx = WavsContext::new(wavs).unwrap();
            let eigen_client = get_eigen_client(ctx).await;

            let name = name.unwrap_or_else(|| Alphanumeric.sample_string(&mut OsRng, 16));

            let signature = run_eth_trigger_echo_task(
                eigen_client.eth,
                watch_wavs,
                ServiceID::new(service_id).unwrap(),
                match workflow_id {
                    Some(workflow_id) => WorkflowID::new(workflow_id).unwrap(),
                    None => WorkflowID::new("default").unwrap(),
                },
                trigger_addr,
                service_manager_addr,
                name,
            )
            .await;

            display_response_signature(&signature);
        }

        Command::KitchenSink {
            add_service,
            wavs,
            register_core_operator,
            register_service_operator,
            digests,
            name,
        } => {
            let ctx = WavsContext::new(wavs).unwrap();
            let eigen_client = get_eigen_client(ctx.clone()).await;
            let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

            if register_core_operator {
                eigen_client
                    .register_operator(&core_contracts)
                    .await
                    .unwrap();
            }

            let avs_client = get_avs_client(&eigen_client, core_contracts.clone()).await;

            if register_service_operator {
                avs_client.register_operator(&mut OsRng).await.unwrap();
            }

            let mut service_id = ServiceID::new("service-id-1").unwrap();
            let workflow_id = WorkflowID::new("default").unwrap();

            if add_service {
                let http_client = HttpClient::new(ctx);

                let digest = match digests.digest_hello_world {
                    None => {
                        let digest = http_client.upload_eth_trigger_echo_digest().await;
                        display_eth_trigger_echo_digest(&digest);
                        digest
                    }
                    Some(digest) => digest,
                };

                service_id = http_client
                    .create_eth_trigger_echo_service(
                        avs_client.layer.trigger,
                        avs_client.layer.service_manager,
                        digest,
                    )
                    .await;
                display_eth_trigger_echo_service_id(&service_id);
            }

            display_core_contracts(&core_contracts);
            display_layer_service_contracts(&avs_client.layer);

            let name = name.unwrap_or_else(|| Alphanumeric.sample_string(&mut OsRng, 16));
            let signature = run_eth_trigger_echo_task(
                eigen_client.eth,
                add_service,
                service_id,
                workflow_id,
                avs_client.layer.trigger,
                avs_client.layer.service_manager,
                name,
            )
            .await;

            display_response_signature(&signature);
        }
    }
}
