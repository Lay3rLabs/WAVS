use alloy::{node_bindings::Anvil, providers::Provider, signers::Signer};
use futures::StreamExt;
use utils::eth_client::{EthClientBuilder, EthClientConfig};

#[tokio::test]
async fn client_stream_blocks() {
    let anvil = Anvil::new().try_spawn().unwrap();

    let config = EthClientConfig {
        ws_endpoint: anvil.ws_endpoint().to_string(),
        http_endpoint: anvil.endpoint().to_string(),
        ..Default::default()
    };

    let builder = EthClientBuilder::new(config);
    let client = builder.build_query().await.unwrap();

    let mut stream = client
        .ws_provider
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
    let anvil = Anvil::new().try_spawn().unwrap();

    let config = EthClientConfig {
        ws_endpoint: anvil.ws_endpoint().to_string(),
        http_endpoint: anvil.endpoint().to_string(),
        mnemonic: Some(
            "work man father plunge mystery proud hollow address reunion sauce theory bonus"
                .to_string(),
        ),
    };

    let builder = EthClientBuilder::new(config);
    let client = builder.build_signing().await.unwrap();

    let message = b"hello world";

    // client.wallet doesn't itself allow signing messages, but we created the wallet from the signer
    let signature = client.signer.sign_message(message).await.unwrap();

    let recovered_address = signature.recover_address_from_msg(&message[..]).unwrap();

    // check that the wallet's default signer is the same as the recovered address
    assert_eq!(recovered_address, client.wallet.default_signer().address());
}
