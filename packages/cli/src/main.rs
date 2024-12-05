mod args;

use args::{CliArgs, Command, DeployArgs};
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{eigen_client::EigenClient, eth_client::{EthClientBuilder, EthClientConfig}, hello_world::HelloWorldFullClientBuilder};

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

    match args.command {
        Command::Deploy(DeployArgs{ws_endpoint, http_endpoint}) => {
            let mnemonic = std::env::var("CLI_ETH_MNEMONIC").expect("CLI_ETH_MNEMONIC env var is required");

            let config = EthClientConfig {
                ws_endpoint,
                http_endpoint,
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

            tracing::info!("Registering as core operator");

            eigen_client
                .register_operator(&core_contracts)
                .await
                .unwrap();

            tracing::info!("Registering as avs operator");
            hello_world_client.register_operator().await.unwrap();

            tracing::info!("Deploying...");
        },
    }
}
