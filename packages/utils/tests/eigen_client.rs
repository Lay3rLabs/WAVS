use alloy::node_bindings::{Anvil, AnvilInstance};
use utils::{
    eigen_client::{CoreAVSAddresses, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
    hello_world::HelloWorldFullClientBuilder,
    init_tracing_tests,
};

#[tokio::test]
async fn deploy_core_contracts() {
    let EigenTestInit { .. } = EigenTestInit::new().await;
}

#[tokio::test]
async fn deploy_hello_world_avs() {
    let EigenTestInit {
        core_contracts,
        eigen_client,
        anvil,
    } = EigenTestInit::new().await;

    eigen_client
        .register_operator(&core_contracts)
        .await
        .unwrap();

    let hello_world_client = HelloWorldFullClientBuilder::new(eigen_client.eth.clone())
        .avs_addresses(core_contracts)
        .build()
        .await
        .unwrap();
    hello_world_client
        .register_operator(&mut rand::rngs::OsRng)
        .await
        .unwrap();
    let hello_world_client = hello_world_client.into_simple();

    // Create and respond first task
    let new_task = hello_world_client
        .create_new_task("foo".to_owned())
        .await
        .unwrap();
    assert_eq!(new_task.taskIndex, 0);
    assert_eq!(new_task.task.name, "foo");
    hello_world_client
        .sign_and_submit_task(new_task.task, new_task.taskIndex)
        .await
        .unwrap();

    // Create and respond second task
    let new_task = hello_world_client
        .create_new_task("bar".to_owned())
        .await
        .unwrap();
    assert_eq!(new_task.taskIndex, 1);
    assert_eq!(new_task.task.name, "bar");
    hello_world_client
        .sign_and_submit_task(new_task.task, new_task.taskIndex)
        .await
        .unwrap();

    // just to make sure we keep anvil alive
    let _ = anvil;
}

#[tokio::test]
async fn register_operator() {
    let EigenTestInit {
        core_contracts,
        eigen_client,
        anvil,
    } = EigenTestInit::new().await;
    eigen_client
        .register_operator(&core_contracts)
        .await
        .unwrap();

    let _ = anvil;
    // TODO
}

struct EigenTestInit {
    pub core_contracts: CoreAVSAddresses,
    pub eigen_client: EigenClient,
    pub anvil: AnvilInstance,
}

impl EigenTestInit {
    pub async fn new() -> Self {
        init_tracing_tests();

        let anvil = Anvil::new().spawn();

        let config = EthClientConfig {
            ws_endpoint: anvil.ws_endpoint().to_string(),
            http_endpoint: anvil.endpoint().to_string(),
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
            hd_index: None,
        };

        let builder = EthClientBuilder::new(config);
        let eth_client = builder.build_signing().await.unwrap();

        let eigen_client = EigenClient::new(eth_client);

        let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();
        Self {
            eigen_client,
            core_contracts,
            anvil,
        }
    }
}

/*

TODO - something like this could theoretically let us use the same anvil instance across all tests
        but there's a problem of conflicts over "nonce"

        Maybe just make all the anvil-based tests run serially?
        Or fix this?

        For now - we're returning a new anvil instance for each test
static ANVIL:OnceLock<AnvilInstance> = OnceLock::new();
static EIGEN_CLIENT:OnceLock<EigenClient> = OnceLock::new();
static INIT: LazyLock<std::sync::Mutex<bool>> = LazyLock::new(|| std::sync::Mutex::new(false));

async fn init() {
    let mut lock = INIT.lock().unwrap();

    // only ever allow setting the anvil instance once
    // using the mutex to prevent access across data races
    if !*lock {
        *lock = true;

        let anvil = Anvil::new().spawn();

        let config = EthClientConfig {
            ws_endpoint: anvil.ws_endpoint().to_string(),
            http_endpoint: anvil.endpoint().to_string(),
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
        };

        let builder = EthClientBuilder::new(config);
        let eth_client = builder.build_signing().await.unwrap();

        let eigen_client = EigenClient::new(eth_client);

        ANVIL.set(anvil).unwrap();
        EIGEN_CLIENT.set(eigen_client).unwrap();
    }
}
async fn get_anvil() -> &'static AnvilInstance {
    init().await;
    ANVIL.get().unwrap()
}

async fn get_eigen_client() -> &'static EigenClient {
    init().await;
    EIGEN_CLIENT.get().unwrap()
}
*/
