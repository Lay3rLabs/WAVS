use serde::Serialize;
use utils::{avs_client::SignedData, eigen_client::CoreAVSAddresses};
use wavs::apis::{ServiceID, WorkflowID};

use crate::deploy::ServiceInfo;

#[derive(Debug)]
pub struct DisplayBuilder {
    pub core_contracts: Option<CoreAVSAddresses>,
    pub service_info: Option<ServiceInfo>,
    pub service_id: Option<ServiceID>,
    pub workflow_id: Option<WorkflowID>,
    pub signed_data: Option<SignedData>,
    pub gas_used: Option<u64>,
}

impl DisplayBuilder {
    pub fn new() -> Self {
        Self {
            core_contracts: None,
            service_info: None,
            service_id: None,
            workflow_id: None,
            signed_data: None,
            gas_used: None,
        }
    }

    pub fn show(self) {
        #[derive(Debug, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub struct DisplayJson {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub core_contracts: Option<CoreAVSAddresses>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub service_info: Option<ServiceInfo>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub service_id: Option<ServiceID>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub workflow_id: Option<WorkflowID>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub signed_data: Option<SignedDataJson>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub gas_used: Option<u64>,
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
                service_info: self.service_info,
                service_id: self.service_id,
                workflow_id: self.workflow_id,
                signed_data,
                gas_used: self.gas_used,
            })
            .unwrap()
        );
    }
}
