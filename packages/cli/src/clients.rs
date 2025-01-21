pub mod example_cosmos_client;
pub mod example_eth_client;

use alloy::sol_types::SolEvent;
use anyhow::{Context, Result};
use layer_climb::prelude::*;
use utils::{
    config::{CosmosChainConfig, EthereumChainConfig},
    eigen_client::EigenClient,
    eth_client::EthClientBuilder,
};
use utils::{ServiceID, WorkflowID};
use wavs::{
    apis::{
        dispatcher::{AllowedHostPermission, Permissions, ServiceConfig, Submit},
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

pub async fn get_eigen_client(
    config: &Config,
    chain_config: EthereumChainConfig,
) -> Result<EigenClient> {
    let client_config = chain_config.to_client_config(None, config.eth_mnemonic.clone(), None);

    let eth_client = EthClientBuilder::new(client_config).build_signing().await?;

    Ok(EigenClient::new(eth_client))
}

pub async fn get_cosmos_client(
    config: &Config,
    chain_config: CosmosChainConfig,
) -> Result<SigningClient> {
    let key_signer = KeySigner::new_mnemonic_str(
        config
            .cosmos_mnemonic
            .as_ref()
            .context("missing mnemonic")?,
        None,
    )?;

    let climb_chain_config: ChainConfig = chain_config.into();
    SigningClient::new(climb_chain_config, key_signer, None).await
}

#[derive(Clone)]
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

    pub async fn get_config(&self) -> Result<wavs::config::Config> {
        self.inner
            .get(format!("{}/config", self.endpoint))
            .send()
            .await?
            .json()
            .await
            .map_err(|e| e.into())
    }

    pub async fn upload_component(&self, wasm_bytes: Vec<u8>) -> Result<Digest> {
        let response: UploadServiceResponse = self
            .inner
            .post(format!("{}/upload", self.endpoint))
            .body(wasm_bytes)
            .send()
            .await?
            .json()
            .await?;

        Ok(response.digest.into())
    }

    pub async fn create_service(
        &self,
        service_info: ServiceInfo,
        digest: Digest,
        config: ServiceConfig,
    ) -> Result<(ServiceID, WorkflowID)> {
        let trigger = match service_info.trigger {
            ServiceTriggerInfo::EthSimpleContract {
                chain_name,
                address,
                event_hash,
            } => {
                if event_hash != *example_eth_client::example_trigger::SimpleTrigger::NewTrigger::SIGNATURE_HASH {
                        tracing::warn!("for right now, we always use a specific event hash... odd for it to be different!");
                    }
                Trigger::eth_contract_event(address, chain_name, event_hash)
            }
            ServiceTriggerInfo::CosmosSimpleContract {
                chain_name,
                address,
                event_type,
            } => {
                if event_type != simple_example_cosmos::event::NewMessageEvent::KEY {
                    tracing::warn!("for right now, we always use a specific event type... odd for it to be different!");
                }
                Trigger::cosmos_contract_event(address, chain_name, event_type)
            }
        };

        let submit = match service_info.submit {
            ServiceSubmitInfo::EigenLayer {
                chain_name,
                avs_addresses,
            } => Submit::EigenContract {
                chain_name,
                service_manager: avs_addresses.service_manager.into(),
                max_gas: config.max_gas,
            },
        };

        let id = ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string())?;

        let service = ServiceRequest {
            trigger,
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
        })?;

        self.inner
            .post(format!("{}/app", self.endpoint))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?
            .error_for_status()?;

        // for now, this is always the WorkflowID - see http service add
        Ok((id, WorkflowID::new("default")?))
    }
}
