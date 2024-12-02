use aggregator::test_utils::app::TestApp;
use alloy::{
    network::{Ethereum, EthereumWallet},
    node_bindings::{Anvil, AnvilInstance},
    primitives::Address,
    providers::{
        fillers::{BlobGasFiller, ChainIdFiller, GasFiller, JoinFill, NonceFiller, WalletFiller},
        Identity, ProviderBuilder, RootProvider, WalletProvider,
    },
    pubsub::PubSubFrontend,
    signers::local::PrivateKeySigner,
    transports::http::{reqwest::Url, Client, Http},
};
use eigen_client_elcontracts::{
    reader::ELChainReader,
    writer::{ELChainWriter, Operator},
};
use eigen_logging::get_logger;
use eigen_utils::{
    delegationmanager::DelegationManager::{self, isOperatorReturn, DelegationManagerInstance},
    get_signer,
};

pub const ANVIL_RPC_URL: &str = "http://localhost:8545";

async fn register_operator(provider: RootProvider<PubSubFrontend>, signer: PrivateKeySigner) {
    let default_slasher = Address::ZERO; // We don't need slasher for our example.
    let default_strategy = Address::ZERO; // We don't need strategy for our example.

    let delegation_manager_address = eigenlayer_deployment()["addresses"]["delegation"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let avs_directory_address: Address = eigenlayer_deployment()["addresses"]["delegation"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let elcontracts_reader_instance = ELChainReader::new(
        get_logger().clone(),
        default_slasher,
        delegation_manager_address,
        avs_directory_address,
        ANVIL_RPC_URL.to_string(),
    );
    let elcontracts_writer_instance = ELChainWriter::new(
        delegation_manager_address,
        default_strategy,
        Address::ZERO,
        elcontracts_reader_instance.clone(),
        ANVIL_RPC_URL.to_string(),
        String::from_utf8(signer.credential().to_bytes().to_vec()).unwrap(),
    );

    let operator = Operator {
        address: signer.address(),
        earnings_receiver_address: signer.address(),
        delegation_approver_address: Address::ZERO,
        staker_opt_out_window_blocks: 0u32,
        metadata_url: None,
    };

    let is_registered = elcontracts_reader_instance
        .is_operator_registered(signer.address())
        .await
        .unwrap();
    get_logger().info(&format!("is registered {}", is_registered), &"");
    #[allow(unused)]
    let tx_hash = elcontracts_writer_instance
        .register_as_operator(operator)
        .await?;
    get_logger().info(
        "Operator registered on EL successfully tx_hash {tx_hash:?}",
        &"",
    );
    let mut salt = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut salt);

    let salt = FixedBytes::from_slice(&salt);
    let now = Utc::now().timestamp();
    let expiry: U256 = U256::from(now + 3600);

    let hello_world_contract_address: Address =
        parse_hello_world_service_manager("contracts/deployments/hello-world/31337.json")?;
    let digest_hash = elcontracts_reader_instance
        .calculate_operator_avs_registration_digest_hash(
            signer.address(),
            hello_world_contract_address,
            salt,
            expiry,
        )
        .await?;

    let signature = signer.sign_hash_sync(&digest_hash)?;
    let operator_signature = SignatureWithSaltAndExpiry {
        signature: signature.as_bytes().into(),
        salt,
        expiry: expiry,
    };
    let stake_registry_address: Address =
        parse_stake_registry_address("contracts/deployments/hello-world/31337.json")?;
    let contract_ecdsa_stake_registry = ECDSAStakeRegistry::new(stake_registry_address, &pr);
    let registeroperator_details_call: alloy::contract::CallBuilder<
        _,
        &_,
        std::marker::PhantomData<ECDSAStakeRegistry::registerOperatorWithSignatureCall>,
        _,
    > = contract_ecdsa_stake_registry
        .registerOperatorWithSignature(operator_signature, signer.clone().address())
        .gas(500000);
    let register_hello_world_hash = registeroperator_details_call
        .send()
        .await?
        .get_receipt()
        .await?
        .transaction_hash;

    get_logger().info(
        &format!(
            "Operator registered on AVS successfully :{} , tx_hash :{}",
            signer.address(),
            register_hello_world_hash
        ),
        &"",
    );

    Ok(())
}

fn eigenlayer_deployment() -> serde_json::Value {
    let eigenlayer_deployment =
        format!("{}/deployments/core/31337.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(eigenlayer_deployment).unwrap()).unwrap()
}

fn hello_world_deployment() -> serde_json::Value {
    let eigenlayer_deployment = format!(
        "{}/deployments/hello-world/31337.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(eigenlayer_deployment).unwrap()).unwrap()
}

#[tokio::test]
async fn operator_registered() {
    let test_app = TestApp::new().await;
    let signing_client = test_app.config.signing_client().await.unwrap();

    let hello_world_deployment = format!(
        "{}/deployments/hello-world/31337.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let hello_world_deployment: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(hello_world_deployment).unwrap()).unwrap();

    let delegation_manager_address: Address = eigenlayer_deployment()["addresses"]["delegation"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let contract_delegation_manager =
        DelegationManager::new(delegation_manager_address, &signing_client.provider);

    let is_operator = contract_delegation_manager
        .isOperator(signing_client.signer.address())
        .call()
        .await
        .unwrap();

    let isOperatorReturn {
        _0: isoperator_bool,
    } = is_operator;

    assert!(isoperator_bool);
}
