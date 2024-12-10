use crate::args::CliArgs;
use layer_climb::prelude::*;
use utils::{
    eigen_client::{CoreAVSAddresses, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
    hello_world::{HelloWorldFullClient, HelloWorldFullClientBuilder},
};
use wavs::{
    apis::{
        dispatcher::{AllowedHostPermission, Permissions, Submit},
        ID,
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
        ws_endpoint: args.ws_endpoint.clone(),
        http_endpoint: args.http_endpoint.clone(),
        mnemonic: Some(mnemonic),
        hd_index: None,
    };

    tracing::info!("Creating eth client on: {:?}", config.ws_endpoint);

    let eth_client = EthClientBuilder::new(config).build_signing().await.unwrap();
    EigenClient::new(eth_client)
}

pub async fn get_avs_client(
    eigen_client: &EigenClient,
    core_contracts: CoreAVSAddresses,
) -> HelloWorldFullClient {
    HelloWorldFullClientBuilder::new(eigen_client.eth.clone())
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

    pub async fn upload_hello_world_digest(&self) -> Digest {
        let wasm_bytes = include_bytes!("../../../components/hello_world.wasm");

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

    pub async fn create_hello_world_service(
        &self,
        address: alloy::primitives::Address,
        erc1271: alloy::primitives::Address,
        digest: Digest,
    ) -> ID {
        self.create_service(
            digest,
            Address::Eth(AddrEth::new(address.into())),
            Address::Eth(AddrEth::new(erc1271.into())),
            Submit::EthSignedMessage { hd_index: 0 },
        )
        .await
    }

    async fn create_service(
        &self,
        digest: Digest,
        task_queue_addr: Address,
        task_queue_erc1271: Address,
        submit: Submit,
    ) -> ID {
        let id = ID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap();

        let service = ServiceRequest {
            trigger: TriggerRequest::eth_queue(task_queue_addr, task_queue_erc1271),
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
