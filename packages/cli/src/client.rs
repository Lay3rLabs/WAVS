use alloy::sol_types::SolEvent;
use layer_climb::prelude::*;
use utils::{
    config::CosmosChainConfig,
    eigen_client::EigenClient,
    eth_client::{EthChainConfig, EthClientBuilder},
    example_eth_client::example_trigger::SimpleTrigger,
};
use utils::{ServiceID, WorkflowID};
use wavs::{
    apis::{
        dispatcher::{AllowedHostPermission, ComponentWorld, Permissions, ServiceConfig, Submit},
        trigger::Trigger,
        ServiceID, WorkflowID,
    },
    http::handlers::service::{
        add::{AddServiceRequest, ServiceRequest},
        upload::UploadServiceResponse,
    },
    Digest,
};

use crate::{
    config::Config,
    deploy::{ServiceInfo, ServiceSubmitInfo, ServiceTriggerInfo},
};

pub async fn get_eigen_client(config: &Config, chain_config: EthChainConfig) -> EigenClient {
    let client_config = chain_config.to_client_config(None, config.eth_mnemonic.clone());

    let eth_client = EthClientBuilder::new(client_config)
        .build_signing()
        .await
        .unwrap();

    EigenClient::new(eth_client)
}

pub async fn get_cosmos_client(config: &Config, chain_config: CosmosChainConfig) -> SigningClient {
    let key_signer =
        KeySigner::new_mnemonic_str(config.cosmos_mnemonic.as_ref().unwrap(), None).unwrap();

    let climb_chain_config: ChainConfig = chain_config.into();
    SigningClient::new(climb_chain_config, key_signer, None)
        .await
        .unwrap()
}

pub struct HttpClient {
    inner: reqwest::Client,
    endpoint: String,
}

impl HttpClient {
    pub fn new(config: &Config) -> Self {
        Self {
            inner: reqwest::Client::new(),
            endpoint: config.wavs_endpoint.clone(),
        }
    }

    pub async fn upload_component(&self, wasm_bytes: Vec<u8>) -> Digest {
        let response: UploadServiceResponse = self
            .inner
            .post(format!("{}/upload", self.endpoint))
            .body(wasm_bytes.to_vec())
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        response.digest.into()
    }

    pub async fn create_service(
        &self,
        service_info: ServiceInfo,
        aggregate: bool,
        digest: Digest,
        config: ServiceConfig,
        world: ComponentWorld,
    ) -> (ServiceID, WorkflowID) {
        let trigger = match service_info.trigger {
            ServiceTriggerInfo::EthSimpleContract {
                chain_name,
                address,
            } => Trigger::eth_contract_event(
                address,
                chain_name,
                SimpleTrigger::NewTrigger::SIGNATURE_HASH,
            ),
            ServiceTriggerInfo::CosmosSimpleContract {
                chain_name,
                address,
            } => Trigger::cosmos_contract_event(
                address,
                chain_name,
                simple_example_cosmos::event::NewMessageEvent::KEY,
            ),
        };

        let submit = match service_info.submit {
            ServiceSubmitInfo::EigenLayer {
                chain_name,
                avs_addresses,
            } => Submit::EigenContract {
                chain_name,
                service_manager: avs_addresses.service_manager.into(),
                aggregate,
                max_gas: config.max_gas,
            },
        };

        let id = ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap();

        let service = ServiceRequest {
            trigger,
            world,
            id: id.clone(),
            digest: digest.into(),
            permissions: Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            },
            testable: Some(true),
            submit,
            config,
        };

        let body = serde_json::to_string(&AddServiceRequest {
            service,
            wasm_url: None,
        })
        .unwrap();

        self.inner
            .post(format!("{}/app", self.endpoint))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();

        // for now, this is always the WorkflowID - see http service add
        (id, WorkflowID::new("default").unwrap())
    }
}
