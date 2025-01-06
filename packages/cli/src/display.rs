use serde::{Deserialize, Serialize};
use utils::{
    eigen_client::CoreAVSAddresses,
    layer_contract_client::{LayerAddresses, SignedData},
};
use wavs::apis::{ServiceID, WorkflowID};

use crate::config::DisplayFormat;

#[derive(Debug)]
pub struct DisplayBuilder {
    pub format: DisplayFormat,
    pub core_contracts: Option<CoreAVSAddresses>,
    pub layer_addresses: Option<LayerAddresses>,
    pub service: Option<ServiceAndWorkflow>,
    pub workflow_id: Option<ServiceID>,
    pub signed_data: Option<SignedData>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServiceAndWorkflow {
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
}

impl DisplayBuilder {
    pub fn new(format: DisplayFormat) -> Self {
        Self {
            format,
            core_contracts: None,
            layer_addresses: None,
            service: None,
            workflow_id: None,
            signed_data: None,
        }
    }

    pub fn show(self) {
        match self.format {
            DisplayFormat::Plaintext => {
                if let Some(core_contracts) = self.core_contracts {
                    display_core_contracts(&core_contracts);
                }
                if let Some(layer_addresses) = self.layer_addresses {
                    display_layer_service_contracts(&layer_addresses);
                }
                if let Some(service) = self.service {
                    display_service(&service);
                }
                if let Some(signed_data) = self.signed_data {
                    display_signed_data(&signed_data);
                }
            }

            DisplayFormat::Json => {
                #[derive(Debug, Serialize)]
                #[serde(rename_all = "snake_case")]
                pub struct DisplayJson {
                    #[serde(skip_serializing_if = "Option::is_none")]
                    pub core_contracts: Option<CoreAVSAddresses>,

                    #[serde(skip_serializing_if = "Option::is_none")]
                    pub layer_addresses: Option<LayerAddresses>,

                    #[serde(skip_serializing_if = "Option::is_none")]
                    pub service: Option<ServiceAndWorkflow>,

                    #[serde(skip_serializing_if = "Option::is_none")]
                    pub workflow_id: Option<ServiceID>,

                    #[serde(skip_serializing_if = "Option::is_none")]
                    pub signed_data: Option<SignedDataJson>,
                }

                #[derive(Debug, Serialize)]
                #[serde(rename_all = "snake_case")]
                pub struct SignedDataJson {
                    pub signature: String,
                    pub data_bytes: String,
                    pub data_utf8: String,
                }

                let signed_data = self.signed_data.map(|signed_data| SignedDataJson {
                    signature: hex::encode(&signed_data.signature),
                    data_bytes: hex::encode(&signed_data.data),
                    data_utf8: String::from_utf8_lossy(&signed_data.data).into(),
                });

                println!(
                    "{}",
                    serde_json::to_string_pretty(&DisplayJson {
                        core_contracts: self.core_contracts,
                        layer_addresses: self.layer_addresses,
                        service: self.service,
                        workflow_id: self.workflow_id,
                        signed_data,
                    })
                    .unwrap()
                );
            }
        }
    }
}

fn display_core_contracts(core_contracts: &CoreAVSAddresses) {
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

fn display_layer_service_contracts(layer_addresses: &LayerAddresses) {
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

fn display_service(service: &ServiceAndWorkflow) {
    println!("\n--- SERVICE ID ---");
    println!("{}", service.service_id);

    println!("\n--- WORKFLOW ID ---");
    println!("{}", service.workflow_id);
}

fn display_signed_data(signed_data: &SignedData) {
    println!("\n--- RESPONSE SIGNATURE ---");
    println!("{}", hex::encode(&signed_data.signature));
    println!("\n--- RESPONSE DATA ---");
    println!("hex encoded: {}", hex::encode(&signed_data.data));
    println!("UTF8: {}", String::from_utf8_lossy(&signed_data.data));
}
