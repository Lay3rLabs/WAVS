use alloy::{
    network::TransactionBuilder,
    node_bindings::Anvil,
    primitives::{utils::parse_ether, Address, U256},
    providers::Provider,
    rpc::types::TransactionRequest,
};
use futures::StreamExt;
use utils::{
    config::EthereumChainConfig,
    eth_client::{
        pool::{BalanceMaintainer, EthSigningClientPoolBuilder},
        EthClientBuilder, EthClientConfig, EthSigningClient,
    },
    init_tracing_tests,
};

#[tokio::test]
async fn client_stream_blocks() {
    init_tracing_tests();
    // seems to be we need to set a block time to get new blocks without explicit transactions?
    let anvil = Anvil::new().block_time_f64(0.02).try_spawn().unwrap();

    let config = EthClientConfig {
        ws_endpoint: Some(anvil.ws_endpoint().to_string()),
        http_endpoint: Some(anvil.endpoint().to_string()),
        ..Default::default()
    };

    let builder = EthClientBuilder::new(config);
    let client = builder.build_query().await.unwrap();

    let mut stream = client
        .provider
        .subscribe_blocks()
        .await
        .unwrap()
        .into_stream();

    let mut counter = 0;

    while counter < 3 {
        let _header = stream.next().await.unwrap();
        counter += 1;
    }
}

#[tokio::test]
async fn signing_pool_basic_same_key() {
    let client_key =
        "planet crucial snake reflect peace prison digital unit shaft garbage rent define"
            .to_string();

    init_tracing_tests();
    inner_signing_pool_basic(None, client_key).await;
}

#[tokio::test]
async fn signing_pool_basic_different_key() {
    let funder_key =
        Some("test test test test test test test test test test test junk".to_string());
    let client_key =
        "planet crucial snake reflect peace prison digital unit shaft garbage rent define"
            .to_string();

    init_tracing_tests();
    inner_signing_pool_basic(funder_key, client_key).await;
}

async fn inner_signing_pool_basic(funder_key: Option<String>, client_key: String) {
    let anvil = Anvil::new().spawn();

    let chain_config = EthereumChainConfig {
        chain_id: anvil.chain_id().to_string(),
        ws_endpoint: Some(anvil.ws_endpoint().to_string()),
        http_endpoint: Some(anvil.endpoint().to_string()),
        aggregator_endpoint: None,
        faucet_endpoint: None,
    };

    let eth_client_pool =
        EthSigningClientPoolBuilder::new(funder_key, client_key, chain_config.clone())
            .with_initial_client_wei(parse_ether("100").unwrap())
            .build()
            .await
            .unwrap();

    let client = eth_client_pool.get().await.unwrap();

    // some other random address to receive funds
    let rando = "0xEf04d5A2D13A792D9D5907c6f1bbc4baE9484069"
        .parse()
        .unwrap();

    // make sure our client has the expected balance
    assert!(balance_approx_eq(&client, parse_ether("100").unwrap()).await);

    // spend some ether
    transfer(&client, rando, parse_ether("25").unwrap()).await;

    // make sure our client has the expected balance
    assert!(balance_approx_eq(&client, parse_ether("75").unwrap()).await);

    // get another client
    let new_client = eth_client_pool.get().await.unwrap();

    // should be different from our previous client
    assert_ne!(new_client.address(), client.address());

    // make sure our client still has the expected balance
    assert!(balance_approx_eq(&client, parse_ether("75").unwrap()).await);

    // new client has fresh balance
    assert!(balance_approx_eq(&new_client, parse_ether("100").unwrap()).await);
}

#[tokio::test]
async fn signing_pool_balance_maintainer() {
    init_tracing_tests();
    let anvil = Anvil::new().spawn();

    let chain_config = EthereumChainConfig {
        chain_id: anvil.chain_id().to_string(),
        ws_endpoint: Some(anvil.ws_endpoint().to_string()),
        http_endpoint: Some(anvil.endpoint().to_string()),
        aggregator_endpoint: None,
        faucet_endpoint: None,
    };

    let top_up_amount = parse_ether("30").unwrap();

    let eth_client_pool = EthSigningClientPoolBuilder::new(
        Some("test test test test test test test test test test test junk".to_string()),
        "planet crucial snake reflect peace prison digital unit shaft garbage rent define"
            .to_string(),
        chain_config.clone(),
    )
    .with_initial_client_wei(parse_ether("100").unwrap())
    .with_balance_maintainer(
        BalanceMaintainer::new(parse_ether("25").unwrap(), top_up_amount).unwrap(),
    )
    .build()
    .await
    .unwrap();

    // just get the address we'll be working with
    let client_address = { eth_client_pool.get().await.unwrap().address() };

    // some other random address to receive funds
    let rando = "0xEf04d5A2D13A792D9D5907c6f1bbc4baE9484069"
        .parse()
        .unwrap();

    {
        let client = eth_client_pool.get().await.unwrap();
        assert_eq!(client.address(), client_address);

        // make sure our client has the expected balance
        assert!(balance_approx_eq(&client, parse_ether("100").unwrap()).await);

        // spend some ether
        transfer(&client, rando, parse_ether("25").unwrap()).await;

        // make sure our client has the expected balance
        assert!(balance_approx_eq(&client, parse_ether("75").unwrap()).await);
    }

    {
        // get the client again
        let client = eth_client_pool.get().await.unwrap();
        assert_eq!(client.address(), client_address);

        // make sure our client has the expected balance (has not been topped up yet)
        assert!(balance_approx_eq(&client, parse_ether("75").unwrap()).await);

        // spend more ether, past the threshhold
        transfer(&client, rando, parse_ether("60").unwrap()).await;

        // make sure our client has the expected balance (has not been topped up yet)
        assert!(balance_approx_eq(&client, parse_ether("15").unwrap()).await);
    }

    {
        // get the client again
        let client = eth_client_pool.get().await.unwrap();
        assert_eq!(client.address(), client_address);

        // now the client is topped up
        assert!(balance_approx_eq(&client, top_up_amount).await);
        // sanity check
        assert_eq!(top_up_amount, parse_ether("30").unwrap());
    }
}

async fn transfer(from: &EthSigningClient, to: Address, wei: U256) {
    let tx = TransactionRequest::default()
        .with_from(from.address())
        .with_to(to)
        .with_value(wei);

    // Send the transaction and listen for the transaction to be included.
    from.provider
        .send_transaction(tx)
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();
}

// check if the balance is equal, with a bit of allowance for gas or whatever
async fn balance_approx_eq(client: &EthSigningClient, expected_balance: U256) -> bool {
    let current_balance = balance(client).await;

    let diff = if current_balance > expected_balance {
        current_balance - expected_balance
    } else {
        expected_balance - current_balance
    };

    let allowed_diff = parse_ether("0.001").unwrap();

    diff < allowed_diff
}

async fn balance(client: &EthSigningClient) -> U256 {
    client.provider.get_balance(client.address()).await.unwrap()
}
