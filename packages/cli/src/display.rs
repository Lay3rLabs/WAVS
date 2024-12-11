use utils::{eigen_client::CoreAVSAddresses, hello_world::config::HelloWorldAddresses};
use wavs::{apis::ID, Digest};

pub fn display_core_contracts(core_contracts: &CoreAVSAddresses, output_file: Option<&str>) {
    println!("\n--- CORE AVS CONTRACTS ---");
    println!(
        "CLI_EIGEN_CORE_PROXY_ADMIN=\"{}\"",
        core_contracts.proxy_admin
    );
    println!(
        "CLI_EIGEN_CORE_DELEGATION_MANAGER=\"{}\"",
        core_contracts.delegation_manager
    );
    println!(
        "CLI_EIGEN_CORE_STRATEGY_MANAGER=\"{}\"",
        core_contracts.strategy_manager
    );
    println!(
        "CLI_EIGEN_CORE_POD_MANAGER=\"{}\"",
        core_contracts.eigen_pod_manager
    );
    println!(
        "CLI_EIGEN_CORE_POD_BEACON=\"{}\"",
        core_contracts.eigen_pod_beacon
    );
    println!(
        "CLI_EIGEN_CORE_PAUSER_REGISTRY=\"{}\"",
        core_contracts.pauser_registry
    );
    println!(
        "CLI_EIGEN_CORE_STRATEGY_FACTORY=\"{}\"",
        core_contracts.strategy_factory
    );
    println!(
        "CLI_EIGEN_CORE_STRATEGY_BEACON=\"{}\"",
        core_contracts.strategy_beacon
    );
    println!(
        "CLI_EIGEN_CORE_AVS_DIRECTORY=\"{}\"",
        core_contracts.avs_directory
    );
    println!(
        "CLI_EIGEN_CORE_REWARDS_COORDINATOR=\"{}\"",
        core_contracts.rewards_coordinator
    );

    if let Some(name) = output_file {
        let curr_dir = std::env::current_dir().unwrap();
        let root = curr_dir.ancestors().nth(2).unwrap();
        let file = std::fs::File::create(root.join(name)).unwrap();
        serde_json::to_writer_pretty(&file, core_contracts).unwrap();
    }
}

pub fn display_hello_world_service_contracts(
    hello_world_service_contracts: &HelloWorldAddresses,
    output_file: Option<&str>,
) {
    println!("\n--- HELLO WORLD AVS CONTRACTS ---");
    println!(
        "CLI_EIGEN_SERVICE_PROXY_ADMIN=\"{}\"",
        hello_world_service_contracts.proxy_admin
    );
    println!(
        "CLI_EIGEN_SERVICE_MANAGER=\"{}\"",
        hello_world_service_contracts.hello_world_service_manager
    );
    println!(
        "CLI_EIGEN_SERVICE_STAKE_REGISTRY=\"{}\"",
        hello_world_service_contracts.stake_registry
    );
    println!(
        "CLI_EIGEN_SERVICE_STAKE_TOKEN=\"{}\"",
        hello_world_service_contracts.token
    );

    if let Some(name) = output_file {
        let curr_dir = std::env::current_dir().unwrap();
        let root = curr_dir.ancestors().nth(2).unwrap();
        let file = std::fs::File::create(root.join(name)).unwrap();
        serde_json::to_writer_pretty(&file, hello_world_service_contracts).unwrap();
    }
}

pub fn display_hello_world_service_id(id: &ID) {
    println!("\n--- HELLO WORLD SERVICE ID ---");
    println!("{}", id);
}

pub fn display_hello_world_digest(digest: &Digest) {
    println!("\n--- HELLO WORLD DIGEST ---");
    println!("CLI_DIGEST_HELLO_WORLD=\"{}\"", digest);
}

pub fn display_task_response_hash(hash: &str) {
    println!("\n--- TASK RESPONSE HASH ---");
    println!("{}", hash);
}
