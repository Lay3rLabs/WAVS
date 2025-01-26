use std::collections::HashMap;

use crate::deploy::ServiceInfo;
use anyhow::Result;
use serde::Serialize;
use utils::{avs_client::SignedData, eigen_client::CoreAVSAddresses, ServiceID, WorkflowID};

#[derive(Debug, Default)]
pub struct DisplayBuilder {
    pub core_contracts: Option<CoreAVSAddresses>,
    pub service: Option<(ServiceID, HashMap<WorkflowID, ServiceInfo>)>,
    pub submit_contract: Option<layer_climb::prelude::Address>,
    pub signed_data: Option<SignedData>,
    pub gas_used: Option<u64>,
}

impl DisplayBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(self) -> Result<()> {
        #[derive(Debug, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub struct DisplayJson {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub core_contracts: Option<CoreAVSAddresses>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub service: Option<(ServiceID, HashMap<WorkflowID, ServiceInfo>)>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub signed_data: Option<SignedDataJson>,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub submit_contract: Option<layer_climb::prelude::Address>,

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

        let display_json = DisplayJson {
            core_contracts: self.core_contracts,
            service: self.service,
            signed_data,
            gas_used: self.gas_used,
            submit_contract: self.submit_contract,
        };

        // For grabbing with tools like `jq` - pure printing without tracing info
        println!("{}", serde_json::to_string_pretty(&display_json)?);

        if let Some((service_id, _)) = display_json.service {
            tracing::info!("Service ID: {}", service_id);
        }
        if let Some(submit_contract) = display_json.submit_contract {
            tracing::info!("Submit contract: {}", submit_contract);
        }

        Ok(())
    }
}
