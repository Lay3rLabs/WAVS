use utils::{
    eigen_client::CoreAVSAddresses,
    layer_contract_client::{LayerAddresses, SignedData},
};
use wavs::apis::ServiceID;

pub fn display_core_contracts(core_contracts: &CoreAVSAddresses) {
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
}

pub fn display_layer_service_contracts(layer_addresses: &LayerAddresses) {
    println!("\n--- LAYER AVS CONTRACTS ---");
    println!(
        "CLI_EIGEN_SERVICE_PROXY_ADMIN=\"{}\"",
        layer_addresses.proxy_admin
    );
    println!(
        "CLI_EIGEN_SERVICE_MANAGER=\"{}\"",
        layer_addresses.service_manager
    );
    println!("CLI_EIGEN_SERVICE_TRIGGER=\"{}\"", layer_addresses.trigger);
    println!(
        "CLI_EIGEN_SERVICE_STAKE_REGISTRY=\"{}\"",
        layer_addresses.stake_registry
    );
    println!(
        "CLI_EIGEN_SERVICE_STAKE_TOKEN=\"{}\"",
        layer_addresses.token
    );
}

pub fn display_service_id(id: &ServiceID) {
    println!("\n--- SERVICE ID ---");
    println!("{}", id);
}

pub fn display_signed_data(signed_data: &SignedData) {
    println!("\n--- RESPONSE SIGNATURE ---");
    println!("{}", hex::encode(&signed_data.signature));
    println!("\n--- RESPONSE DATA ---");
    println!("hex encoded: {}", hex::encode(&signed_data.data));
    println!("UTF8: {}", String::from_utf8_lossy(&signed_data.data));
}
