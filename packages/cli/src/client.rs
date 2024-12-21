use crate::args::CliArgs;
use layer_climb::prelude::*;
use utils::{
    eigen_client::{CoreAVSAddresses, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
    layer_contract_client::{LayerContractClientFull, LayerContractClientFullBuilder},
};
use wavs::{
    apis::{
        dispatcher::{AllowedHostPermission, Permissions, Submit},
        ServiceID,
    },
    http::{
        handlers::service::{
            add::{AddServiceRequest, ServiceRequest},
            upload::UploadServiceResponse,
        },
        types::TriggerRequest,
    },
    Digest,
};

pub async fn get_eigen_client(args: &CliArgs) -> EigenClient {
    let mnemonic = std::env::var("CLI_ETH_MNEMONIC").expect("CLI_ETH_MNEMONIC env var is required");

    let config = EthClientConfig {
        chain_id: args.chain_id.clone(),
        ws_endpoint: Some(args.ws_endpoint.clone()),
        http_endpoint: args.http_endpoint.clone(),
        mnemonic: Some(mnemonic),
        hd_index: None,
        transport: None,
    };

    tracing::info!("Creating eth client on: {:?}", config.ws_endpoint);

    let eth_client = EthClientBuilder::new(config).build_signing().await.unwrap();
    EigenClient::new(eth_client)
}

pub async fn get_avs_client(
    eigen_client: &EigenClient,
    core_contracts: CoreAVSAddresses,
) -> LayerContractClientFull {
    LayerContractClientFullBuilder::new(eigen_client.eth.clone())
        .avs_addresses(core_contracts)
        .build()
        .await
        .unwrap()
}

pub struct HttpClient {
    inner: reqwest::Client,
    endpoint: String,
}

impl HttpClient {
    pub fn new(args: &CliArgs) -> Self {
        Self {
            inner: reqwest::Client::new(),
            endpoint: args.wavs_endpoint.clone(),
        }
    }

    pub async fn upload_eth_trigger_echo_digest(&self) -> Digest {
        let wasm_bytes = include_bytes!("../../../components/eth_trigger_echo.wasm");

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

    pub async fn create_eth_trigger_echo_service(
        &self,
        trigger_address: alloy::primitives::Address,
        service_manager_address: alloy::primitives::Address,
        chain_id: String,
        digest: Digest,
    ) -> ServiceID {
        self.create_service(
            digest,
            Address::Eth(AddrEth::new(trigger_address.into())),
            Submit::EthSignedMessage {
                chain_id,
                hd_index: 0,
                service_manager_addr: Address::Eth(AddrEth::new(service_manager_address.into())),
            },
        )
        .await
    }

    async fn create_service(
        &self,
        digest: Digest,
        trigger_address: Address,
        submit: Submit,
    ) -> ServiceID {
        let id = ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap();

        let service = ServiceRequest {
            trigger: TriggerRequest::eth_event(trigger_address),
            id: id.clone(),
            digest: digest.into(),
            permissions: Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            },
            envs: Vec::new(),
            testable: Some(true),
            submit,
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

        id
    }
}
