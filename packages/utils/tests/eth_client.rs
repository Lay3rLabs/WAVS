use alloy_node_bindings::Anvil;
use alloy_provider::Provider;
use alloy_signer::Signer;
use futures::StreamExt;
use utils::{
    evm_client::{EvmQueryClient, EvmSigningClient, EvmSigningClientConfig},
    init_tracing_tests,
};
use wavs_types::Credential;

#[tokio::test]
async fn client_stream_blocks() {
    init_tracing_tests();
    // seems to be we need to set a block time to get new blocks without explicit transactions?
    let anvil = Anvil::new().block_time_f64(0.02).try_spawn().unwrap();

    let client = EvmQueryClient::new(anvil.ws_endpoint().parse().unwrap())
        .await
        .unwrap();

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
async fn client_sign_message() {
    init_tracing_tests();
    let anvil = Anvil::new().spawn();

    let config = EvmSigningClientConfig::new(
        anvil.endpoint().parse().unwrap(),
        Credential::new(
            "work man father plunge mystery proud hollow address reunion sauce theory bonus"
                .to_string(),
        ),
    );
    let client = EvmSigningClient::new(config).await.unwrap();

    let message = b"hello world";

    // client.wallet doesn't itself allow signing messages, but we created the wallet from the signer
    let signature = client.signer.sign_message(message).await.unwrap();

    let recovered_address = signature.recover_address_from_msg(&message[..]).unwrap();

    // check that the wallet's default signer is the same as the recovered address
    assert_eq!(recovered_address, client.wallet.default_signer().address());
}
