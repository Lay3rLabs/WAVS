// Currently - e2e tests are disabled by default.
// See TESTS.md for more information on how to run e2e tests.

#[cfg(feature = "e2e_tests")]
mod e2e {
    mod cosmos;
    mod eth;
    mod http;

    use std::{path::PathBuf, sync::Arc, time::Duration};

    use alloy::node_bindings::{Anvil, AnvilInstance};
    use anyhow::bail;
    use cosmos::CosmosTestApp;
    use eth::EthTestApp;
    use http::HttpClient;
    use lavs_apis::{
        events::{task_queue_events::TaskCreatedEvent, traits::TypedEvent},
        tasks as task_queue,
    };
    use layer_climb::prelude::*;
    use serde::{Deserialize, Serialize};
    use utils::layer_contract_client::LayerContractClientSimple;
    use wavs::{
        apis::{dispatcher::Submit, ServiceID},
        http::types::TriggerRequest,
        test_utils::app::TestApp,
    };
    use wavs::{config::Config, dispatcher::CoreDispatcher, AppContext, Digest};

    #[test]
    fn e2e_tests() {
        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_ethereum")] {
                // should match the wavs.toml
                let anvil = Some(Anvil::new().port(8545u16).chain_id(31337).spawn());
                let anvil2 = Some(Anvil::new().port(8645u16).chain_id(31338).spawn());
            } else {
                let anvil: Option<AnvilInstance> = None;
                let anvil2: Option<AnvilInstance> = None;
            }
        }
        let mut config = {
            tokio::runtime::Runtime::new().unwrap().block_on({
                async {
                    let mut cli_args = TestApp::default_cli_args();
                    cli_args.dotenv = None;
                    cli_args.data = Some(tempfile::tempdir().unwrap().path().to_path_buf());
                    cli_args.home = Some(PathBuf::from(".."));
                    TestApp::new_with_args(cli_args)
                        .await
                        .config
                        .as_ref()
                        .clone()
                }
            })
        };

        let aggregator_config: aggregator::config::Config = {
            let mut cli_args = aggregator::test_utils::app::TestApp::default_cli_args();
            cli_args.dotenv = None;
            cli_args.data = Some(tempfile::tempdir().unwrap().path().to_path_buf());
            if let Some(anvil) = anvil.as_ref() {
                cli_args.ws_endpoint = Some(anvil.ws_endpoint().to_string());
                cli_args.http_endpoint = Some(anvil.endpoint().to_string());
            }
            aggregator::config::ConfigBuilder::new(cli_args)
                .build()
                .unwrap()
        };

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_cosmos")] {
                config.cosmos_chain = Some("layer-local".to_string());
            } else {
                config.cosmos_chain = None;
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "e2e_tests_ethereum")] {
                config.eth_chains = vec!["local".to_string(), "local2".to_string()];
            } else {
                config.eth_chains = Vec::new();
            }
        }

        let ctx = AppContext::new();

        let dispatcher = Arc::new(CoreDispatcher::new_core(&config).unwrap());

        let wavs_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = config.clone();
            move || {
                wavs::run_server(ctx, config, dispatcher);
            }
        });

        let aggregator_handle = std::thread::spawn({
            let config = aggregator_config.clone();
            let ctx = ctx.clone();
            move || {
                aggregator::run_server(ctx, config);
            }
        });

        let test_handle = std::thread::spawn({
            move || {
                ctx.rt.clone().block_on({
                    async move {
                        let http_client = HttpClient::new(&config);

                        // give the server a bit of time to start
                        tokio::time::timeout(Duration::from_secs(2), async {
                            loop {
                                match http_client.get_config().await {
                                    Ok(_) => break,
                                    Err(_) => {
                                        tracing::info!("Waiting for server to start...");
                                        tokio::time::sleep(Duration::from_millis(100)).await;
                                    }
                                }
                            }
                        })
                        .await
                        .unwrap();

                        let digests = Digests::new(http_client.clone());
                        let service_ids = ServiceIds::new();

                        match (config.cosmos_chain.is_some(), !config.eth_chains.is_empty()) {
                            (true, false) => {
                                run_tests_cosmos(http_client, config, digests, service_ids).await
                            }
                            (false, true) => {
                                run_tests_ethereum(
                                    config.eth_chains[0].clone(),
                                    config.eth_chains[1].clone(),
                                    #[allow(clippy::unnecessary_literal_unwrap)]
                                    anvil.unwrap(),
                                    #[allow(clippy::unnecessary_literal_unwrap)]
                                    anvil2.unwrap(),
                                    http_client,
                                    config,
                                    digests,
                                    service_ids,
                                )
                                .await;
                            }
                            (true, true) => {
                                run_tests_crosschain(http_client, config, digests, service_ids)
                                    .await
                            }
                            (false, false) => panic!(
                                "No chain selected at all for e2e tests (see e2e_tests_* features)"
                            ),
                        }
                        ctx.kill();
                    }
                });
            }
        });

        test_handle.join().unwrap();
        wavs_handle.join().unwrap();
        aggregator_handle.join().unwrap();
    }

    async fn run_tests_ethereum(
        chain_name: String,
        chain_name2: String,
        anvil: AnvilInstance,
        anvil2: AnvilInstance,
        http_client: HttpClient,
        config: Config,
        digests: Digests,
        service_ids: ServiceIds,
    ) {
        tracing::info!("Running e2e ethereum tests");

        let app = EthTestApp::new(config.clone(), anvil).await;
        let app_2 = EthTestApp::new(config.clone(), anvil2).await;

        let avs_trigger_addr = Address::Eth(AddrEth::new(app.avs_client.layer.trigger.into()));
        let avs_trigger_addr_2 = Address::Eth(AddrEth::new(app_2.avs_client.layer.trigger.into()));

        let avs_service_manager_addr =
            Address::Eth(AddrEth::new(app.avs_client.layer.service_manager.into()));
        let avs_service_manager_addr_2 =
            Address::Eth(AddrEth::new(app_2.avs_client.layer.service_manager.into()));

        let avs_client: LayerContractClientSimple = app.avs_client.into();
        let avs_client_2: LayerContractClientSimple = app_2.avs_client.into();

        let trigger_echo_digest = digests.eth_trigger_echo_digest().await;
        let trigger_square_digest = digests.eth_trigger_square_digest().await;
        let cosmos_query_digest = digests.eth_cosmos_query().await;

        let trigger_echo_service_id = service_ids.eth_trigger_echo();
        let trigger_echo_service_id_2 = service_ids.eth_trigger_echo_2();
        let trigger_echo_aggregate_service_id = service_ids.eth_trigger_echo_aggregate();
        let trigger_square_service_id = service_ids.eth_trigger_square();
        let cosmos_query_service_id = service_ids.eth_cosmos_query();

        for (service_id, digest, is_aggregate, is_second_ethereum) in [
            (
                trigger_echo_service_id.clone(),
                trigger_echo_digest.clone(),
                false,
                false,
            ),
            (
                trigger_echo_service_id_2.clone(),
                trigger_echo_digest.clone(),
                false,
                true,
            ),
            (
                trigger_echo_aggregate_service_id.clone(),
                trigger_echo_digest,
                true,
                false,
            ),
            (
                trigger_square_service_id.clone(),
                trigger_square_digest,
                false,
                false,
            ),
            (
                cosmos_query_service_id.clone(),
                cosmos_query_digest,
                false,
                false,
            ),
        ] {
            let (avs_client, avs_trigger_addr, avs_service_manager_addr, chain_name) =
                match is_second_ethereum {
                    false => (
                        &avs_client,
                        &avs_trigger_addr,
                        &avs_service_manager_addr,
                        &chain_name,
                    ),
                    true => (
                        &avs_client_2,
                        &avs_trigger_addr_2,
                        &avs_service_manager_addr_2,
                        &chain_name2,
                    ),
                };

            if service_id.is_some() {
                let service_id = service_id.unwrap();
                let digest = digest.unwrap();

                http_client
                    .create_service(
                        service_id.clone(),
                        digest,
                        TriggerRequest::eth_event(avs_trigger_addr.clone()),
                        Submit::EthSignedMessage {
                            chain_name: chain_name.to_string(),
                            hd_index: 0,
                            service_manager_addr: avs_service_manager_addr.clone(),
                        },
                    )
                    .await
                    .unwrap();

                tracing::info!("Service created: {}", service_id);

                if is_aggregate {
                    http_client
                        .register_service_on_aggregator(
                            chain_name,
                            avs_client.service_manager_contract_address,
                            service_id.clone(),
                            &config,
                        )
                        .await
                        .unwrap();
                }
            }
        }

        if let Some(service_id) = trigger_echo_service_id {
            tracing::info!("Submitting trigger_echo task...");
            let echo_trigger_id = avs_client
                .trigger
                .add_trigger(
                    service_id.to_string(),
                    "default".to_string(),
                    b"foo".to_vec(),
                )
                .await
                .unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let avs_client = avs_client.clone();
                async move {
                    loop {
                        let signed_data =
                            avs_client.load_signed_data(echo_trigger_id).await.unwrap();
                        match signed_data {
                            Some(signed_data) => {
                                tracing::info!("(endpoint: {}) GOT THE SIGNATURE!", avs_client.eth.config.ws_endpoint.as_ref().unwrap());
                                tracing::info!("{}", hex::encode(signed_data.signature));
                                break;
                            }
                            None => {
                                tracing::info!(
                                    "(endpoint: {}) Waiting for task response by {} on {} for trigger_id {}...",
                                    avs_client.eth.config.ws_endpoint.as_ref().unwrap(),
                                    avs_client.eth.address(),
                                    avs_client.service_manager_contract_address,
                                    echo_trigger_id
                                );
                            }
                        }
                        // still open, waiting...
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            })
            .await
            .unwrap();
        }

        if let Some(service_id) = trigger_echo_service_id_2 {
            tracing::info!("Submitting trigger_echo task...");
            let echo_trigger_id = avs_client_2
                .trigger
                .add_trigger(
                    service_id.to_string(),
                    "default".to_string(),
                    b"foo".to_vec(),
                )
                .await
                .unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let avs_client = avs_client_2.clone();
                async move {
                    loop {
                        let signed_data =
                            avs_client.load_signed_data(echo_trigger_id).await.unwrap();
                        match signed_data {
                            Some(signed_data) => {
                                tracing::info!("(endpoint: {}) GOT THE SIGNATURE!", avs_client.eth.config.ws_endpoint.as_ref().unwrap());
                                tracing::info!("{}", hex::encode(signed_data.signature));
                                break;
                            }
                            None => {
                                tracing::info!(
                                    "(endpoint: {}) Waiting for task response by {} on {} for trigger_id {}...",
                                    avs_client.eth.config.ws_endpoint.as_ref().unwrap(),
                                    avs_client.eth.address(),
                                    avs_client.service_manager_contract_address,
                                    echo_trigger_id
                                );
                            }
                        }
                        // still open, waiting...
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            })
            .await
            .unwrap();
        }

        if let Some(service_id) = trigger_square_service_id {
            tracing::info!("Submitting square task...");
            let square_trigger_id = avs_client
                .trigger
                .add_trigger(
                    service_id.to_string(),
                    "default".to_string(),
                    serde_json::to_vec(&SquareRequest { x: 3 }).unwrap(),
                )
                .await
                .unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let avs_client = avs_client.clone();
                async move {
                    loop {
                        let signed_data = avs_client
                            .load_signed_data(square_trigger_id)
                            .await
                            .unwrap();
                        match signed_data {
                            Some(signed_data) => {
                                tracing::info!("(endpoint: {}) GOT THE SIGNATURE!", avs_client.eth.config.ws_endpoint.as_ref().unwrap());
                                tracing::info!("{}", hex::encode(signed_data.signature));

                                let response =
                                    serde_json::from_slice::<SquareResponse>(&signed_data.data)
                                        .unwrap();

                                tracing::info!("GOT THE RESPONSE!");
                                tracing::info!("{:?}", response);
                                break;
                            }
                            None => {
                                tracing::info!(
                                    "(endpoint: {}) Waiting for task response by {} on {} for trigger_id {}...",
                                    avs_client.eth.config.ws_endpoint.as_ref().unwrap(),
                                    avs_client.eth.address(),
                                    avs_client.service_manager_contract_address,
                                    square_trigger_id
                                );
                            }
                        }
                        // still open, waiting...
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            })
            .await
            .unwrap();
        }

        if let Some(service_id) = trigger_echo_aggregate_service_id {
            let echo_aggregate_trigger_id_1 = avs_client
                .trigger
                .add_trigger(
                    service_id.to_string(),
                    "default".to_string(),
                    b"foo-aggregate".to_vec(),
                )
                .await
                .unwrap();

            let echo_aggregate_trigger_id_2 = avs_client
                .trigger
                .add_trigger(
                    service_id.to_string(),
                    "default".to_string(),
                    b"bar-aggregate".to_vec(),
                )
                .await
                .unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let avs_client = avs_client.clone();
                async move {
                    loop {
                        let signed_data_1 = avs_client
                            .load_signed_data(echo_aggregate_trigger_id_1)
                            .await
                            .unwrap();

                        let signed_data_2 = avs_client
                            .load_signed_data(echo_aggregate_trigger_id_2)
                            .await
                            .unwrap();

                        match (signed_data_1, signed_data_2) {
                            (Some(signed_data_1), Some(signed_data_2)) => {
                                tracing::info!(
                                    "(endpoint: {}) GOT THE AGGREGATED SIGNATURES!",
                                    avs_client.eth.config.ws_endpoint.as_ref().unwrap()
                                );
                                tracing::info!("1: {}", hex::encode(signed_data_1.signature));
                                tracing::info!("2: {}", hex::encode(signed_data_2.signature));
                                break;
                            }
                            (None, Some(_)) => {
                                tracing::info!("Got aggregation #1, waiting for #2...");
                            }
                            (Some(_), None) => {
                                tracing::info!("Got aggregation #2, waiting for #1...");
                            }
                            (None, None) => {
                                tracing::info!("Waiting for aggregation responses...");
                            }
                        }
                        // still open, waiting...
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            })
            .await
            .unwrap();
        }

        if let Some(service_id) = cosmos_query_service_id {
            tracing::info!("Submitting cosmos query tasks...");
            let trigger_id = avs_client
                .trigger
                .add_trigger(
                    service_id.to_string(),
                    "default".to_string(),
                    serde_json::to_vec(&CosmosQueryRequest::BlockHeight).unwrap(),
                )
                .await
                .unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let avs_client = avs_client.clone();
                async move {
                    loop {
                        let signed_data = avs_client.load_signed_data(trigger_id).await.unwrap();
                        match signed_data {
                            Some(signed_data) => {
                                tracing::info!("GOT THE SIGNATURE!");
                                tracing::info!("{}", hex::encode(signed_data.signature));

                                let response = serde_json::from_slice::<CosmosQueryResponse>(
                                    &signed_data.data,
                                )
                                .unwrap();

                                tracing::info!("GOT THE RESPONSE!");
                                match response {
                                    CosmosQueryResponse::BlockHeight(height) => {
                                        tracing::info!("height: {}", height);
                                    }
                                    _ => panic!("Expected block height"),
                                }

                                break;
                            }
                            None => {
                                tracing::info!(
                                    "Waiting for task response by {} on {} for trigger_id {}...",
                                    avs_client.eth.address(),
                                    avs_client.service_manager_contract_address,
                                    trigger_id
                                );
                            }
                        }
                        // still open, waiting...
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            })
            .await
            .unwrap();

            let trigger_id = avs_client
                .trigger
                .add_trigger(
                    service_id.to_string(),
                    "default".to_string(),
                    serde_json::to_vec(&CosmosQueryRequest::Balance {
                        // this test expects that we're running on Starship
                        // https://github.com/cosmology-tech/starship/blob/5635e853ac9e364f0ae9c87646536c30b6519748/starship/charts/devnet/configs/keys.json#L27
                        address: Address::new_cosmos_string(
                            "osmo1pss7nxeh3f9md2vuxku8q99femnwdjtc8ws4un",
                            None,
                        )
                        .unwrap(),
                    })
                    .unwrap(),
                )
                .await
                .unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let avs_client = avs_client.clone();
                async move {
                    loop {
                        let signed_data = avs_client.load_signed_data(trigger_id).await.unwrap();
                        match signed_data {
                            Some(signed_data) => {
                                tracing::info!("GOT THE SIGNATURE!");
                                tracing::info!("{}", hex::encode(signed_data.signature));

                                let response = serde_json::from_slice::<CosmosQueryResponse>(
                                    &signed_data.data,
                                )
                                .unwrap();

                                tracing::info!("GOT THE RESPONSE!");
                                match response {
                                    CosmosQueryResponse::Balance(balance) => {
                                        tracing::info!("balance: {}", balance);
                                    }
                                    _ => panic!("Expected balance"),
                                }

                                break;
                            }
                            None => {
                                tracing::info!(
                                    "Waiting for task response by {} on {} for trigger_id {}...",
                                    avs_client.eth.address(),
                                    avs_client.service_manager_contract_address,
                                    trigger_id
                                );
                            }
                        }
                        // still open, waiting...
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            })
            .await
            .unwrap();
        }
    }

    async fn run_tests_crosschain(
        _http_client: HttpClient,
        _config: Config,
        _digests: Digests,
        _service_ids: ServiceIds,
    ) {
        tracing::info!("Running e2e crosschain tests");
        // TODO!
    }

    async fn run_tests_cosmos(
        http_client: HttpClient,
        config: Config,
        digests: Digests,
        service_ids: ServiceIds,
    ) {
        tracing::info!("Running e2e cosmos tests");

        let app = CosmosTestApp::new(config).await;

        if let Some(service_id) = service_ids.cosmos_permissions() {
            let wasm_digest = digests.permissions_digest().await.unwrap();

            http_client
                .create_service(
                    service_id.clone(),
                    wasm_digest,
                    TriggerRequest::LayerQueue {
                        task_queue_addr: app.task_queue.addr.clone(),
                        poll_interval: 1000,
                        hd_index: 0,
                    },
                    Submit::LayerVerifierTx {
                        hd_index: 0,
                        verifier_addr: app.verifier_addr.clone(),
                    },
                )
                .await
                .unwrap();

            tracing::info!("Service created: {}", service_id);

            let tx_resp = app
                .task_queue
                .submit_task(
                    "example request",
                    PermissionsExampleRequest {
                        url: "https://httpbin.org/get".to_string(),
                    },
                )
                .await
                .unwrap();

            let event: TaskCreatedEvent = CosmosTxEvents::from(&tx_resp)
                .event_first_by_type(TaskCreatedEvent::NAME)
                .map(cosmwasm_std::Event::from)
                .unwrap()
                .try_into()
                .unwrap();

            tracing::info!("Task created: {}", event.task_id);

            let timeout = tokio::time::timeout(Duration::from_secs(3), async move {
                loop {
                    let task = app.task_queue.query_task(event.task_id).await.unwrap();
                    match task.status {
                        task_queue::Status::Open {} => {
                            // still open, waiting...
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                        task_queue::Status::Completed { .. } => return Ok(task),
                        task_queue::Status::Expired {} => bail!("Task expired"),
                    }
                }
            })
            .await;

            match timeout {
                Ok(task) => {
                    let task = task.unwrap();
                    let result = task.result.unwrap();
                    tracing::info!("task completed!");
                    tracing::info!("result: {:#?}", result);
                }
                Err(_) => panic!("Timeout waiting for task to complete"),
            }

            tracing::info!("regular task submission past, running test service..");

            let result: PermissionsExampleResponse = http_client
                .test_service(
                    &service_id,
                    PermissionsExampleRequest {
                        url: "https://httpbin.org/get".to_string(),
                    },
                )
                .await
                .unwrap();

            tracing::info!("success!");
            assert!(result.filecount > 0);
            tracing::info!("{:#?}", result);
        }
    }

    #[derive(Deserialize, Serialize, Debug)]
    struct PermissionsExampleRequest {
        pub url: String,
    }

    #[derive(Deserialize, Serialize, Debug)]
    struct PermissionsExampleResponse {
        pub filename: PathBuf,
        pub contents: String,
        pub filecount: usize,
    }

    #[derive(Serialize, Debug)]
    pub struct SquareRequest {
        pub x: u64,
    }

    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    pub struct SquareResponse {
        pub y: u64,
    }

    #[derive(Serialize, Debug)]
    #[serde(rename_all = "snake_case")]
    pub enum CosmosQueryRequest {
        BlockHeight,
        Balance { address: Address },
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "snake_case")]
    pub enum CosmosQueryResponse {
        BlockHeight(u64),
        Balance(String),
    }

    pub struct ServiceIds {}

    impl ServiceIds {
        pub fn new() -> Self {
            Self {}
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_square")]
        pub fn eth_trigger_square(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-trigger-square").unwrap())
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_echo")]
        pub fn eth_trigger_echo(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-trigger-echo").unwrap())
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_echo")]
        pub fn eth_trigger_echo_2(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-trigger-echo-2").unwrap())
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_echo_aggregate")]
        pub fn eth_trigger_echo_aggregate(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-trigger-echo-aggregate").unwrap())
        }

        #[cfg(feature = "e2e_tests_ethereum_cosmos_query")]
        pub fn eth_cosmos_query(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-cosmos-query").unwrap())
        }

        #[cfg(feature = "e2e_tests_cosmos_permissions")]
        pub fn cosmos_permissions(&self) -> Option<ServiceID> {
            Some(ServiceID::new("cosmos-permissions").unwrap())
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_square"))]
        pub fn eth_trigger_square(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_echo"))]
        pub fn eth_trigger_echo(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_echo"))]
        pub fn eth_trigger_echo_2(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_echo_aggregate"))]
        pub fn eth_trigger_echo_aggregate(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_cosmos_query"))]
        pub fn eth_cosmos_query(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_cosmos_permissions"))]
        pub fn cosmos_permissions(&self) -> Option<ServiceID> {
            None
        }
    }

    pub struct Digests {
        http_client: HttpClient,
    }

    impl Digests {
        pub fn new(http_client: HttpClient) -> Self {
            Self { http_client }
        }

        #[cfg(feature = "e2e_tests_cosmos_permissions")]
        pub async fn permissions_digest(&self) -> Option<Digest> {
            self.get_digest("WAVS_E2E_PERMISSIONS_WASM_DIGEST", "permissions")
                .await
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_square")]
        pub async fn eth_trigger_square_digest(&self) -> Option<Digest> {
            self.get_digest(
                "WAVS_E2E_ETH_TRIGGER_SQUARE_WASM_DIGEST",
                "eth_trigger_square",
            )
            .await
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_echo")]
        pub async fn eth_trigger_echo_digest(&self) -> Option<Digest> {
            self.get_digest("WAVS_E2E_ETH_TRIGGER_ECHO_WASM_DIGEST", "eth_trigger_echo")
                .await
        }

        #[cfg(feature = "e2e_tests_ethereum_cosmos_query")]
        pub async fn eth_cosmos_query(&self) -> Option<Digest> {
            self.get_digest("WAVS_E2E_ETH_COSMOS_QUERY_WASM_DIGEST", "eth_cosmos_query")
                .await
        }

        #[cfg(not(feature = "e2e_tests_cosmos_permissions"))]
        pub async fn permissions_digest(&self) -> Option<Digest> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_square"))]
        pub async fn eth_trigger_square_digest(&self) -> Option<Digest> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_echo"))]
        pub async fn eth_trigger_echo_digest(&self) -> Option<Digest> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_cosmos_query"))]
        pub async fn eth_cosmos_query(&self) -> Option<Digest> {
            None
        }

        async fn get_digest(&self, env_var_key: &str, wasm_filename: &str) -> Option<Digest> {
            let digest = std::env::var(env_var_key);

            let digest: Digest = match digest {
                Ok(digest) => digest.parse().unwrap(),
                Err(_) => {
                    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .join("components")
                        .join(format!("{}.wasm", wasm_filename));

                    tracing::info!("Uploading wasm: {}", wasm_path.display());

                    let wasm_bytes = tokio::fs::read(wasm_path).await.unwrap();
                    self.http_client
                        .upload_wasm(wasm_bytes.to_vec())
                        .await
                        .unwrap()
                }
            };

            Some(digest)
        }
    }
}
