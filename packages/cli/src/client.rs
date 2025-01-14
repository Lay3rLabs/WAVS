use layer_climb::prelude::*;
use utils::{
    avs_client::{AvsClient, AvsClientBuilder, ServiceManagerDeps}, config::CosmosChainConfig, eigen_client::{CoreAVSAddresses, EigenClient}, eth_client::{EthChainConfig, EthClientBuilder}
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

pub async fn try_get_eigen_client(config: &Config) -> Option<EigenClient> {
    let chain_config = config.eth_chain.as_ref().map(|chain| {
        config
            .chains
            .get_chain(chain)
            .unwrap()
            .unwrap_or_else(|| panic!("chain not found for {}", chain))
    });

    let chain_config = match chain_config {
        Some(chain_config) => chain_config,
        None => {
            return None;
        }
    };

    match EthChainConfig::try_from(chain_config) {
        Ok(chain_config) => {
            let client_config = chain_config.to_client_config(None, config.eth_mnemonic.clone());

            let eth_client = EthClientBuilder::new(client_config)
                .build_signing()
                .await
                .unwrap();
            Some(EigenClient::new(eth_client))
        }
        Err(e) => {
            None
        }
    }
}

pub async fn try_get_cosmos_client(config: &Config) -> Option<SigningClient> {
    let chain_config = config.cosmos_chain.as_ref().map(|chain| {
        config
            .chains
            .get_chain(chain)
            .unwrap()
            .unwrap_or_else(|| panic!("chain not found for {}", chain))
    });

    let chain_config = match chain_config {
        Some(chain_config) => chain_config,
        None => {
            return None;
        }
    };

    match CosmosChainConfig::try_from(chain_config) {
        Ok(chain_config) => {
            let key_signer = KeySigner::new_mnemonic_str(config.cosmos_mnemonic.as_ref().unwrap(), None).unwrap();

            let climb_chain_config: ChainConfig = chain_config.into();
            let signing_client = SigningClient::new(climb_chain_config, key_signer, None)
                .await
                .unwrap();

            Some(signing_client)
        }
        Err(e) => {
            None
        }
    }
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
        trigger_chain_name: impl ToString,
        trigger_address: Address,
        submit_chain_name: impl ToString,
        submit_address: Address,
        digest: Digest,
        config: ServiceConfig,
        world: ComponentWorld,
    ) -> (ServiceID, WorkflowID) {
        let submit = Submit::EigenContract {
            chain_name: submit_chain_name.to_string(),
            service_manager: submit_address,
            aggregate: false,
            max_gas: config.max_gas,
        };

        let id = ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap();

        let service = ServiceRequest {
            trigger: Trigger::contract_event(trigger_address, trigger_chain_name.to_string()),
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
