use alloy_node_bindings::Anvil;
use alloy_provider::Provider;
use alloy_signer::Signer;
use futures::StreamExt;
use utils::{
    evm_client::{EvmQueryClient, EvmSigningClient, EvmSigningClientConfig},
    init_tracing_tests,
    test_utils::anvil::safe_spawn_anvil,
};
use wavs_types::Credential;

#[tokio::test]
async fn client_sign_message() {
    init_tracing_tests();
    let anvil = safe_spawn_anvil();

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
