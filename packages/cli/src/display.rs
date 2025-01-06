use serde::{Deserialize, Serialize};
use utils::{
    eigen_client::CoreAVSAddresses,
    layer_contract_client::{LayerAddresses, SignedData},
};
use wavs::apis::{ServiceID, WorkflowID};

#[derive(Debug)]
pub struct DisplayBuilder {
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
    pub fn new() -> Self {
        Self {
            core_contracts: None,
            layer_addresses: None,
            service: None,
            workflow_id: None,
            signed_data: None,
        }
    }

    pub fn show(self) {
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
