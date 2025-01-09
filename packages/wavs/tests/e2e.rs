// Currently - e2e tests are disabled by default.
// See TESTS.md for more information on how to run e2e tests.

#[cfg(feature = "e2e_tests")]
mod e2e {
    // TODO
    //mod cosmos;
    mod eth;
    mod http;

    use std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
        sync::Arc,
        time::Duration,
    };

    use alloy::node_bindings::{Anvil, AnvilInstance};
    use eth::EthTestApp;
    use http::HttpClient;
    use layer_climb::prelude::*;
    use serde::{Deserialize, Serialize};
    use utils::{avs_client::ServiceManagerClient, config::ConfigBuilder};
    use wavs::{
        apis::{
            dispatcher::{ComponentWorld, Submit},
            trigger::Trigger,
            ServiceID,
        },
        test_utils::app::TestApp,
    };
    use wavs::{config::Config, dispatcher::CoreDispatcher, AppContext, Digest};

    fn workspace_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn wavs_path() -> PathBuf {
        workspace_path().join("packages").join("wavs")
    }

    fn aggregator_path() -> PathBuf {
        workspace_path().join("packages").join("aggregator")
    }

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
                    let mut cli_args = TestApp::zeroed_cli_args();
                    cli_args.home = Some(wavs_path());
                    cli_args.dotenv = None;
                    TestApp::new_with_args(cli_args)
                        .await
                        .config
                        .as_ref()
                        .clone()
                }
            })
        };

        let aggregator_config: aggregator::config::Config = {
            let mut cli_args = aggregator::test_utils::app::TestApp::zeroed_cli_args();
            cli_args.home = Some(aggregator_path());
            cli_args.dotenv = None;
            cli_args.chain = Some("local".to_string());
            ConfigBuilder::new(cli_args).build().unwrap()
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

        let trigger_echo_digest = digests.eth_trigger_echo_digest().await;
        let trigger_square_digest = digests.eth_trigger_square_digest().await;
        let cosmos_query_digest = digests.eth_cosmos_query().await;

        let trigger_echo_service_id_1 = service_ids.eth_trigger_echo_1();
        let trigger_echo_service_id_2 = service_ids.eth_trigger_echo_2();
        let trigger_echo_aggregate_service_id = service_ids.eth_trigger_echo_aggregate();
        let trigger_square_service_id = service_ids.eth_trigger_square();
        let cosmos_query_service_id = service_ids.eth_cosmos_query();

        let mut clients = HashMap::new();
        let mut contract_addrs = HashSet::new();

        for (service_id, digest, world, is_aggregate, is_second_ethereum) in [
            (
                trigger_echo_service_id_1.clone(),
                trigger_echo_digest.clone(),
                ComponentWorld::ChainEvent,
                false,
                false,
            ),
            (
                trigger_echo_service_id_2.clone(),
                trigger_echo_digest.clone(),
                ComponentWorld::ChainEvent,
                false,
                true,
            ),
            (
                trigger_echo_aggregate_service_id.clone(),
                trigger_echo_digest,
                ComponentWorld::ChainEvent,
                true,
                false,
            ),
            (
                trigger_square_service_id.clone(),
                trigger_square_digest,
                ComponentWorld::ChainEvent,
                false,
                false,
            ),
            (
                cosmos_query_service_id.clone(),
                cosmos_query_digest,
                ComponentWorld::ChainEvent,
                false,
                false,
            ),
        ] {
            if service_id.is_some() {
                let service_id = service_id.unwrap();
                let digest = digest.unwrap();

                let (trigger_client, submit_client) = match is_second_ethereum {
                    false => app.deploy_service_contracts().await,
                    true => app_2.deploy_service_contracts().await,
                };

                let chain_name = match is_second_ethereum {
                    false => chain_name.clone(),
                    true => chain_name2.clone(),
                };

                let app_name = match is_second_ethereum {
                    false => "app",
                    true => "app_2",
                };

                if !contract_addrs.insert((app_name, trigger_client.contract_address.clone())) {
                    panic!(
                        "({app_name}) ({service_id}) Duplicate trigger contract address: {}",
                        trigger_client.contract_address
                    );
                }
                if !contract_addrs.insert((app_name, submit_client.contract_address.clone())) {
                    panic!(
                        "({app_name}) ({service_id}) Duplicate submit contract address: {}",
                        submit_client.contract_address
                    );
                }

                let trigger_contract_address = Address::Eth(
                    AddrEth::new_vec(trigger_client.contract_address.to_vec()).unwrap(),
                );
                let submit_contract_address = Address::Eth(
                    AddrEth::new_vec(submit_client.contract_address.to_vec()).unwrap(),
                );

                http_client
                    .create_service(
                        service_id.clone(),
                        digest,
                        Trigger::contract_event(
                            trigger_contract_address.clone(),
                            chain_name.clone(),
                        ),
                        Submit::eigen_contract(
                            chain_name.to_string(),
                            submit_contract_address.clone(),
                            false, // FIXME, use is_aggregate: https://github.com/Lay3rLabs/WAVS/issues/254
                            None
                        ),
                        world,
                    )
                    .await
                    .unwrap();

                tracing::info!("Service created: {}", service_id);

                if is_aggregate {
                    http_client
                        .register_service_on_aggregator(
                            &chain_name,
                            submit_client.contract_address.clone(),
                            &config,
                        )
                        .await
                        .unwrap();
                }

                if clients
                    .insert(service_id.clone(), (trigger_client, submit_client))
                    .is_some()
                {
                    panic!("Duplicate service id: {}", service_id);
                }
            }
        }

        if let Some(service_id) = trigger_echo_service_id_1 {
            let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
            tracing::info!("Submitting trigger_echo task...");
            let echo_trigger_id = trigger_client.add_trigger(b"foo".to_vec()).await.unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let submit_client = submit_client.clone();
                async move {
                    loop {
                        if submit_client.trigger_validated(echo_trigger_id).await {
                            break;
                        } else {
                            tracing::info!(
                                "Waiting on response for service {}, trigger {}",
                                service_id,
                                echo_trigger_id
                            );
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
            let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
            tracing::info!("Submitting trigger_echo task...");
            let echo_trigger_id = trigger_client.add_trigger(b"foo".to_vec()).await.unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let submit_client = submit_client.clone();
                async move {
                    loop {
                        if submit_client.trigger_validated(echo_trigger_id).await {
                            break;
                        } else {
                            tracing::info!(
                                "Waiting on response for service {}, trigger {}",
                                service_id,
                                echo_trigger_id
                            );
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
            let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
            tracing::info!("Submitting square task...");
            let square_trigger_id = trigger_client
                .add_trigger(serde_json::to_vec(&SquareRequest { x: 3 }).unwrap())
                .await
                .unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let submit_client = submit_client.clone();
                async move {
                    loop {
                        let data = submit_client.trigger_data(square_trigger_id).await.ok();

                        match data {
                            Some(data) => {
                                println!("{:?}", data);
                                let response =
                                    serde_json::from_slice::<SquareResponse>(&data).unwrap();

                                tracing::info!("GOT THE RESPONSE!");
                                tracing::info!("{:?}", response);
                                break;
                            }
                            None => {
                                tracing::info!(
                                    "Waiting on response for service {}, trigger {}",
                                    service_id,
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
            let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
            let echo_aggregate_trigger_id_1 = trigger_client
                .add_trigger(b"foo-aggregate".to_vec())
                .await
                .unwrap();

            let echo_aggregate_trigger_id_2 = trigger_client
                .add_trigger(b"bar-aggregate".to_vec())
                .await
                .unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let submit_client = submit_client.clone();
                async move {
                    loop {
                        let signature_1 = submit_client
                            .trigger_data(echo_aggregate_trigger_id_1)
                            .await
                            .ok();

                        let signature_2 = submit_client
                            .trigger_data(echo_aggregate_trigger_id_2)
                            .await
                            .ok();

                        match (signature_1, signature_2) {
                            (Some(signature_1), Some(signature_2)) => {
                                tracing::info!("GOT THE AGGREGATED SIGNATURES!",);
                                tracing::info!("1: {}", hex::encode(signature_1));
                                tracing::info!("2: {}", hex::encode(signature_2));
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
            let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
            tracing::info!("Submitting cosmos query tasks...");
            let trigger_id = trigger_client
                .add_trigger(serde_json::to_vec(&CosmosQueryRequest::BlockHeight).unwrap())
                .await
                .unwrap();

            tokio::time::timeout(Duration::from_secs(10), {
                let submit_client = submit_client.clone();
                let service_id = service_id.clone();
                async move {
                    loop {
                        let data = submit_client.trigger_data(trigger_id).await.ok();
                        match data {
                            Some(data) => {
                                let response =
                                    serde_json::from_slice::<CosmosQueryResponse>(&data).unwrap();

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
                                    "Waiting on response for service {}, trigger {}",
                                    service_id,
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

            let trigger_id = trigger_client
                .add_trigger(
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
                let submit_client = submit_client.clone();
                async move {
                    loop {
                        let data = submit_client.trigger_data(trigger_id).await.ok();
                        match data {
                            Some(data) => {
                                let response =
                                    serde_json::from_slice::<CosmosQueryResponse>(&data).unwrap();

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
                                    "Waiting on response for service {}, trigger {}",
                                    service_id,
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
        /*
        tracing::info!("Running e2e cosmos tests");

        let app = CosmosTestApp::new(config).await;

        if let Some(service_id) = service_ids.cosmos_permissions() {
            let wasm_digest = digests.permissions_digest().await.unwrap();

            http_client
                .create_service(
                    service_id.clone(),
                    wasm_digest,
                    Trigger::contract_event(app.task_queue.addr.clone()),
                    Submit::CosmosContract { chain_name: "foo".to_string(), contract_addr: app.verifier_addr.clone() },
                    ComponentWorld::ChainEvent,
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
        */
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

        #[cfg(feature = "e2e_tests_ethereum_trigger_echo_1")]
        pub fn eth_trigger_echo_1(&self) -> Option<ServiceID> {
            Some(ServiceID::new("eth-trigger-echo-1").unwrap())
        }

        #[cfg(feature = "e2e_tests_ethereum_trigger_echo_2")]
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

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_echo_1"))]
        pub fn eth_trigger_echo_1(&self) -> Option<ServiceID> {
            None
        }

        #[cfg(not(feature = "e2e_tests_ethereum_trigger_echo_2"))]
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
            self.get_digest("WAVS_E2E_ETH_TRIGGER_SQUARE_WASM_DIGEST", "square")
                .await
        }

        #[cfg(any(
            feature = "e2e_tests_ethereum_trigger_echo_1",
            feature = "e2e_tests_ethereum_trigger_echo_2"
        ))]
        pub async fn eth_trigger_echo_digest(&self) -> Option<Digest> {
            self.get_digest("WAVS_E2E_ETH_TRIGGER_ECHO_WASM_DIGEST", "echo_data")
                .await
        }

        #[cfg(feature = "e2e_tests_ethereum_cosmos_query")]
        pub async fn eth_cosmos_query(&self) -> Option<Digest> {
            self.get_digest("WAVS_E2E_ETH_COSMOS_QUERY_WASM_DIGEST", "cosmos_query")
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

        #[cfg(not(any(
            feature = "e2e_tests_ethereum_trigger_echo_1",
            feature = "e2e_tests_ethereum_trigger_echo_2"
        )))]
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
                        .join("examples")
                        .join("build")
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
