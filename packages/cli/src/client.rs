use layer_climb::prelude::*;
use utils::{
    avs_client::{AvsClient, AvsClientBuilder, ServiceManagerDeps},
    eigen_client::{CoreAVSAddresses, EigenClient},
    eth_client::{EthChainConfig, EthClientBuilder},
};
use utils::{ServiceID, WorkflowID};
use wavs::{
    apis::{
        dispatcher::{AllowedHostPermission, ComponentWorld, ServiceConfig, Permissions, Submit},
        trigger::Trigger,
        ServiceID, WorkflowID,
    },
    http::handlers::service::{
        add::{AddServiceRequest, ServiceRequest},
        upload::UploadServiceResponse,
    },
    Digest,
};

use crate::config::Config;

pub async fn get_eigen_client(config: &Config) -> EigenClient {
    let chain_config = config
        .chains
        .get_chain(&config.chain)
        .unwrap()
        .unwrap_or_else(|| panic!("chain not found for {}", config.chain));
    let chain_config = EthChainConfig::try_from(chain_config).unwrap();
    let client_config = chain_config.to_client_config(None, config.eth_mnemonic.clone());

    let eth_client = EthClientBuilder::new(client_config)
        .build_signing()
        .await
        .unwrap();
    EigenClient::new(eth_client)
}

pub async fn get_avs_client<F, Fut>(
    eigen_client: &EigenClient,
    core_contracts: CoreAVSAddresses,
    service_manager_override: Option<alloy::primitives::Address>,
    deploy_service_manager: F,
) -> AvsClient
where
    F: FnOnce(ServiceManagerDeps) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<alloy::primitives::Address>>,
{
    AvsClientBuilder::new(eigen_client.eth.clone())
        .core_addresses(core_contracts)
        .override_service_manager(service_manager_override)
        .build(deploy_service_manager)
        .await
        .unwrap()
}

pub struct HttpClient {
    inner: reqwest::Client,
    endpoint: String,
    chain_name: String,
}

impl HttpClient {
    pub fn new(config: &Config) -> Self {
        Self {
            inner: reqwest::Client::new(),
            endpoint: config.wavs_endpoint.clone(),
            chain_name: config.chain.clone(),
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
        trigger_address: alloy::primitives::Address,
        service_manager_address: alloy::primitives::Address,
        digest: Digest,
        config: ServiceConfig,
        world: ComponentWorld,
        chain_id: impl ToString,
    ) -> (ServiceID, WorkflowID) {
        let trigger_address = trigger_address.into(); 
        let submit = Submit::EigenContract {
            chain_name: self.chain_name.clone(),
            max_gas: config.max_gas,
            service_manager: service_manager_address.into() 
            aggregate: false,
        };

        let id = ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap();

        let service = ServiceRequest {
            trigger: Trigger::contract_event(trigger_address, chain_id),
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
