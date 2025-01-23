use alloy::{node_bindings::Anvil, primitives::Address};
use wavs_aggregator::test_utils::app::TestApp;

// tests that we load chain config section correctly
#[tokio::test]
async fn config_mnemonic() {
    let anvil = Anvil::new().spawn();
    let config = TestApp::new_with_args(TestApp::zeroed_cli_args(), Some(&anvil)).config;

    let signer = config.signer().unwrap();
    assert_eq!(
        signer.address(),
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            .parse::<Address>()
            .unwrap()
    );

    // change the mnemonic via cli
    let mut cli_args = TestApp::zeroed_cli_args();
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_owned();
    cli_args.mnemonic = Some(mnemonic);
    let config = TestApp::new_with_args(cli_args, None).config;
    let signer2 = config.signer().unwrap();
    assert_eq!(
        signer2.address(),
        "0x9858effd232b4033e47d90003d41ec34ecaeda94"
            .parse::<Address>()
            .unwrap()
    );
}
