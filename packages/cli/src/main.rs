mod args;
mod client;
mod display;
mod task;

use args::{CliArgs, Command};
use clap::Parser;
use client::{get_avs_client, get_eigen_client, HttpClient};
use display::{
    display_core_contracts, display_hello_world_digest, display_hello_world_service_contracts,
    display_hello_world_service_id, display_task_response_hash,
};
use rand::{
    distributions::{Alphanumeric, DistString},
    rngs::OsRng,
};
use task::run_hello_world_task;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let args = CliArgs::parse();

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

    match args.command.clone() {
        Command::DeployCore { register_operator } => {
            let eigen_client = get_eigen_client(&args).await;
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
            wavs,
            core_contracts,
            register_operator,
            digests,
        } => {
            let core_contracts = core_contracts.into();

            let eigen_client = get_eigen_client(&args).await;
            let avs_client = get_avs_client(&eigen_client, core_contracts).await;

            if register_operator {
                avs_client.register_operator(&mut OsRng).await.unwrap();
            }

            if wavs {
                let http_client = HttpClient::new(&args);

                let digest = match digests.digest_hello_world {
                    None => {
                        let digest = http_client.upload_hello_world_digest().await;
                        display_hello_world_digest(&digest);
                        digest
                    }
                    Some(digest) => digest,
                };

                let service_id = http_client
                    .create_hello_world_service(
                        avs_client.hello_world.hello_world_service_manager,
                        digest,
                    )
                    .await;
                display_hello_world_service_id(&service_id);
            }

            display_hello_world_service_contracts(&avs_client.hello_world);
        }

        Command::DeployAll {
            wavs,
            register_core_operator,
            register_service_operator,
            digests,
        } => {
            let eigen_client = get_eigen_client(&args).await;
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

            if wavs {
                let http_client = HttpClient::new(&args);

                let digest = match digests.digest_hello_world {
                    None => {
                        let digest = http_client.upload_hello_world_digest().await;
                        display_hello_world_digest(&digest);
                        digest
                    }
                    Some(digest) => digest,
                };

                let service_id = http_client
                    .create_hello_world_service(
                        avs_client.hello_world.hello_world_service_manager,
                        digest,
                    )
                    .await;
                display_hello_world_service_id(&service_id);
            }

            display_core_contracts(&core_contracts);
            display_hello_world_service_contracts(&avs_client.hello_world);
        }

        Command::AddTask {
            wavs,
            contract_address,
            name,
        } => {
            let eigen_client = get_eigen_client(&args).await;

            let name = name.unwrap_or_else(|| Alphanumeric.sample_string(&mut OsRng, 16));

            let hash = run_hello_world_task(eigen_client.eth, wavs, contract_address, name).await;

            display_task_response_hash(&hash);
        }

        Command::KitchenSink {
            wavs,
            register_core_operator,
            register_service_operator,
            digests,
            name,
        } => {
            let eigen_client = get_eigen_client(&args).await;
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

            if wavs {
                let http_client = HttpClient::new(&args);

                let digest = match digests.digest_hello_world {
                    None => {
                        let digest = http_client.upload_hello_world_digest().await;
                        display_hello_world_digest(&digest);
                        digest
                    }
                    Some(digest) => digest,
                };

                let service_id = http_client
                    .create_hello_world_service(
                        avs_client.hello_world.hello_world_service_manager,
                        digest,
                    )
                    .await;
                display_hello_world_service_id(&service_id);
            }

            display_core_contracts(&core_contracts);
            display_hello_world_service_contracts(&avs_client.hello_world);

            let name = name.unwrap_or_else(|| Alphanumeric.sample_string(&mut OsRng, 16));
            let hash = run_hello_world_task(
                eigen_client.eth,
                wavs,
                avs_client.hello_world.hello_world_service_manager,
                name,
            )
            .await;

            display_task_response_hash(&hash);
        }
    }
}
