mod args;

use anyhow::Result;
use args::{CliArgs, Command};
use clap::Parser;
use lavs_apis::id::TaskId;
use rand::rngs::OsRng;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    eigen_client::{CoreAVSAddresses, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
    hello_world::{
        config::HelloWorldAddresses,
        solidity_types::hello_world::HelloWorldServiceManager::NewTaskCreated,
        HelloWorldFullClient, HelloWorldFullClientBuilder, HelloWorldSimpleClient,
    },
};

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

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

    match args.command.clone() {
        Command::Deploy => {
            DeployData::new(&args).await.unwrap();
        }

        Command::KitchenSink { task_message } => {
            let DeployData {
                core_addresses,
                hello_world_client,
                eigen_client,
            } = DeployData::new(&args).await.unwrap();

            tracing::info!("Registering as core operator");

            eigen_client
                .register_operator(&core_addresses)
                .await
                .unwrap();

            tracing::info!("Registering as avs operator");
            hello_world_client
                .register_operator(&mut OsRng)
                .await
                .unwrap();

            println!(
                "Registered as operator with address {}",
                eigen_client.eth.address()
            );

            tracing::info!("Submitting a hello world task");

            let hello_world_client = hello_world_client.into_simple();

            let NewTaskCreated { task, taskIndex } = hello_world_client
                .create_new_task(task_message)
                .await
                .unwrap();

            println!("Task submitted with id: {}", TaskId::new(taskIndex as u64));

            tracing::info!("Submitting the hello world result");

            let tx_hash = hello_world_client
                .sign_and_submit_task(task, taskIndex)
                .await
                .unwrap();

            println!("Task result submitted with tx hash: {}", tx_hash);
        }
    }
}

struct DeployData {
    pub core_addresses: CoreAVSAddresses,
    pub hello_world_client: HelloWorldFullClient,
    pub eigen_client: EigenClient,
}

impl DeployData {
    async fn new(args: &CliArgs) -> Result<Self> {
        let mnemonic =
            std::env::var("CLI_ETH_MNEMONIC").expect("CLI_ETH_MNEMONIC env var is required");

        let config = EthClientConfig {
            ws_endpoint: args.ws_endpoint.clone(),
            http_endpoint: args.http_endpoint.clone(),
            mnemonic: Some(mnemonic),
            hd_index: None,
        };

        tracing::info!("Creating eth client on: {:?}", config.ws_endpoint);

        let eth_client = EthClientBuilder::new(config).build_signing().await.unwrap();
        let eigen_client = EigenClient::new(eth_client);

        let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

        println!("--- CORE CONTRACTS ---");
        println!("{:#?}", core_contracts);

        let hello_world_client = HelloWorldFullClientBuilder::new(eigen_client.eth.clone())
            .avs_addresses(core_contracts.clone())
            .build()
            .await
            .unwrap();

        println!("--- HELLO WORLD AVS CONTRACTS ---");
        println!("{:#?}", hello_world_client.hello_world);

        Ok(Self {
            core_addresses: core_contracts,
            hello_world_client: hello_world_client,
            eigen_client,
        })
    }
}
