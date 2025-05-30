use alloy_node_bindings::Anvil;
use alloy_primitives::{FixedBytes, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_signer::{k256::ecdsa::SigningKey, SignerSync};
use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
use alloy_sol_types::sol;

use crate::{Envelope, EnvelopeExt, EnvelopeSignature, IWavsServiceManager};

// Define the simple service manager contract for testing
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    SimpleServiceManager,
    "../../examples/contracts/solidity/abi/SimpleServiceManager.sol/SimpleServiceManager.json"
);

fn mock_signer() -> LocalSigner<SigningKey> {
    MnemonicBuilder::<English>::default()
        .word_count(24)
        .build_random()
        .unwrap()
}

#[tokio::test]
async fn test_validate_function() {
    // Set up test environment with Anvil
    let anvil = Anvil::new().spawn();
    let wallet: LocalSigner<SigningKey> = anvil.keys()[0].clone().into();
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(anvil.endpoint_url());

    // Deploy a simple service manager contract
    let service_manager = SimpleServiceManager::deploy(provider.clone())
        .await
        .unwrap();

    // Set up test signers
    let signer_1 = mock_signer();
    let signer_2 = mock_signer();

    // Configure operator weights and thresholds
    const NUM_SIGNERS: usize = 2;
    const NUM_THRESHOLD: usize = 2;

    service_manager
        .setLastCheckpointTotalWeight(U256::from(NUM_SIGNERS as u64))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    service_manager
        .setLastCheckpointThresholdWeight(U256::from(NUM_THRESHOLD as u64))
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // Set operator weights
    service_manager
        .setOperatorWeight(signer_1.address(), U256::ONE)
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    service_manager
        .setOperatorWeight(signer_2.address(), U256::ONE)
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // Create test envelope directly using the correct type
    let envelope = Envelope {
        eventId: FixedBytes([0; 20]),
        ordering: FixedBytes([0; 12]),
        payload: alloy_primitives::Bytes::from_static(&[1, 2, 3]),
    };

    // Create signatures for both signers
    let signature_1 =
        EnvelopeSignature::Secp256k1(signer_1.sign_hash_sync(&envelope.eip191_hash()).unwrap());
    let signature_2 =
        EnvelopeSignature::Secp256k1(signer_2.sign_hash_sync(&envelope.eip191_hash()).unwrap());

    // Get current block height
    let block_height = provider.get_block_number().await.unwrap();

    // Create signature data with both signers (meeting threshold)
    let signatures = vec![signature_1.clone(), signature_2.clone()];
    let signature_data = envelope.signature_data(signatures, block_height).unwrap();

    // Create the service manager instance for validation
    let service_manager_for_validation =
        IWavsServiceManager::new(*service_manager.address(), provider.clone());

    // Test: validate function should succeed with sufficient quorum
    let result = service_manager_for_validation
        .validate(envelope.clone().into(), signature_data.clone().into())
        .call()
        .await;

    assert!(
        result.is_ok(),
        "Validation should succeed with sufficient quorum"
    );

    // Test: validate function should fail with insufficient quorum (only one signer)
    let insufficient_signatures = vec![signature_1.clone()];
    let insufficient_signature_data = envelope
        .signature_data(insufficient_signatures, block_height)
        .unwrap();

    let insufficient_result = service_manager_for_validation
        .validate(envelope.clone().into(), insufficient_signature_data.into())
        .call()
        .await;

    assert!(
        insufficient_result.is_err(),
        "Validation should fail with insufficient quorum"
    );
}

#[tokio::test]
async fn test_validate_function_invalid_signature() {
    // Set up test environment with Anvil
    let anvil = Anvil::new().spawn();
    let wallet: LocalSigner<SigningKey> = anvil.keys()[0].clone().into();
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(anvil.endpoint_url());

    // Deploy a simple service manager contract
    let service_manager = SimpleServiceManager::deploy(provider.clone())
        .await
        .unwrap();

    // Set up test signer
    let signer = mock_signer();
    let unauthorized_signer = mock_signer();

    // Configure operator weights and thresholds
    service_manager
        .setLastCheckpointTotalWeight(U256::ONE)
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    service_manager
        .setLastCheckpointThresholdWeight(U256::ONE)
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // Set operator weight only for authorized signer
    service_manager
        .setOperatorWeight(signer.address(), U256::ONE)
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();

    // Create test envelope and signature with unauthorized signer
    let envelope = Envelope {
        eventId: FixedBytes([0; 20]),
        ordering: FixedBytes([0; 12]),
        payload: alloy_primitives::Bytes::from_static(&[1, 2, 3]),
    };

    let unauthorized_signature = EnvelopeSignature::Secp256k1(
        unauthorized_signer
            .sign_hash_sync(&envelope.eip191_hash())
            .unwrap(),
    );

    // Get current block height
    let block_height = provider.get_block_number().await.unwrap();

    // Create signature data with unauthorized signer
    let signatures = vec![unauthorized_signature];
    let signature_data = envelope.signature_data(signatures, block_height).unwrap();

    // Create the service manager instance for validation
    let service_manager_for_validation =
        IWavsServiceManager::new(*service_manager.address(), provider.clone());

    // Test: validate function should fail with unauthorized signer
    let result = service_manager_for_validation
        .validate(envelope.into(), signature_data.into())
        .call()
        .await;

    assert!(
        result.is_err(),
        "Validation should fail with unauthorized signer"
    );
}
