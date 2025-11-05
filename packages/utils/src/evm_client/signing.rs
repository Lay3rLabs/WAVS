use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_rpc_types_eth::TransactionReceipt;
use alloy_signer::k256::SecretKey;
use alloy_signer_local::{coins_bip39::English, MnemonicBuilder, PrivateKeySigner};
use std::time::Duration;
use tokio::time::sleep;
use wavs_types::{Credential, Envelope, SignatureData};

use crate::{error::EvmClientError, evm_client::AnyNonceManager};

use super::EvmSigningClient;

// Transaction retry configuration
const MAX_RETRIES: u32 = 3;
const BASE_RETRY_DELAY_MS: u64 = 100;

pub fn make_signer(
    credentials: &Credential,
    hd_index: Option<u32>,
) -> super::Result<PrivateKeySigner> {
    let hd_index = hd_index.unwrap_or_default();

    match credentials.strip_prefix("0x") {
        Some(stripped) => {
            // if the string begins with `0x`, it is a private key
            // and so we can't derive additional keys from it
            if hd_index > 0 {
                return Err(EvmClientError::DerivationWithPrivateKey.into());
            }
            let private_key = const_hex::decode(stripped)?;
            let secret_key = SecretKey::from_slice(&private_key)?;
            Ok(PrivateKeySigner::from_signing_key(secret_key.into()))
        }
        None => Ok(MnemonicBuilder::<English>::default()
            .phrase(credentials.as_str())
            .index(hd_index)?
            .build()?),
    }
}

impl EvmSigningClient {
    pub async fn send_envelope_signatures(
        &self,
        envelope: Envelope,
        signature_data: SignatureData,
        service_handler: Address,
        max_gas: Option<u64>,
        gas_price: Option<u128>,
    ) -> Result<TransactionReceipt, EvmClientError> {
        if self
            .provider
            .get_code_at(service_handler)
            .await
            .map_err(|e| EvmClientError::FailedGetCode(service_handler, e.into()))?
            .is_empty()
        {
            return Err(EvmClientError::NotContract(service_handler));
        }

        let gas = match max_gas {
            None => {
                let gas_estimate = self
                    .service_handler(service_handler)
                    .handleSignedEnvelope(envelope.clone(), signature_data.clone())
                    .estimate_gas()
                    .await
                    .map_err(|e| EvmClientError::TransactionWithoutReceipt(e.into()))?;

                // pad it with a multiplier to account for gas fluctuations
                ((gas_estimate as f32) * self.gas_estimate_multiplier()) as u64
            }

            Some(gas) => {
                // EIP-1559 has a default 30m gas limit per block without override. Else:
                // 'a intrinsic gas too high -- tx.gas_limit > env.block.gas_limit' is thrown
                gas.min(30_000_000)
            }
        };

        let service_handler_instance = self.service_handler(service_handler);
        let mut tx_builder = service_handler_instance
            .handleSignedEnvelope(envelope, signature_data)
            .gas(gas);

        // Set gas price if provided
        if let Some(price) = gas_price {
            tx_builder = tx_builder.gas_price(price);
        }

        let mut retry_count = 0;

        let receipt = loop {
            let send_result = tx_builder.send().await;

            match send_result {
                Ok(pending_tx) => {
                    break pending_tx
                        .get_receipt()
                        .await
                        .map_err(|e| EvmClientError::TransactionWithoutReceipt(e.into()))?;
                }
                Err(e) => {
                    if retry_count >= MAX_RETRIES {
                        return Err(EvmClientError::SendTransaction(e.into()));
                    }

                    retry_count += 1;

                    let error_msg = e.to_string().to_lowercase();
                    let is_nonce_error = error_msg.contains("replacement transaction underpriced")
                        || error_msg.contains("nonce");

                    if is_nonce_error {
                        tracing::warn!(
                            "Nonce error detected (attempt {}/{}), refreshing nonce and retrying: {}",
                            retry_count, MAX_RETRIES, e
                        );

                        // Refresh nonce if using FastNonceManager
                        if let AnyNonceManager::Fast(fast_nonce_manager) = &self.nonce_manager {
                            if fast_nonce_manager
                                .set_current_nonce(&self.provider)
                                .await
                                .is_ok()
                            {
                                // Add exponential backoff delay after nonce refresh
                                let delay_ms = BASE_RETRY_DELAY_MS * (1 << (retry_count - 1));
                                sleep(Duration::from_millis(delay_ms)).await;
                                continue;
                            }
                        }
                    } else {
                        tracing::warn!(
                            "Transaction failed (attempt {}/{}), retrying: {}",
                            retry_count,
                            MAX_RETRIES,
                            e
                        );

                        // Add exponential backoff delay for network-related retries
                        let delay_ms = BASE_RETRY_DELAY_MS * (1 << (retry_count - 1));
                        sleep(Duration::from_millis(delay_ms)).await;
                        continue;
                    }
                }
            }
        };

        match receipt.status() {
            true => Ok(receipt),
            false => Err(EvmClientError::TransactionWithReceipt(Box::new(receipt))),
        }
    }
}

#[cfg(test)]
mod test {
    use alloy_primitives::{Bytes, FixedBytes};
    use alloy_provider::Provider;
    use alloy_rpc_types_eth::TransactionTrait;
    use alloy_signer_local::{coins_bip39::English, MnemonicBuilder, PrivateKeySigner};
    use wavs_types::{Credential, Envelope, EnvelopeSigner, SignatureKind};

    use crate::{
        evm_client::{AnyNonceManager, EvmSigningClient, EvmSigningClientConfig},
        test_utils::test_contracts::TestContractDeps,
    };

    mod test_service_contracts {
        pub mod manager {
            use alloy_sol_types::sol;

            sol!(
                #[allow(missing_docs)]
                #[sol(rpc)]
                TestServiceManager,
                "../../examples/contracts/solidity/abi/TestServiceContracts.sol/TestServiceManager.json"
            );
        }

        pub mod handler {
            use alloy_sol_types::sol;

            sol!(
                #[allow(missing_docs)]
                #[sol(rpc)]
                TestServiceHandler,
                "../../examples/contracts/solidity/abi/TestServiceContracts.sol/TestServiceHandler.json"
            );
        }
    }

    use test_service_contracts::{handler::TestServiceHandler, manager::TestServiceManager};

    #[tokio::test]
    async fn signature_validation() {
        let signer = mock_signer();
        let envelope = mock_envelope();

        let signature = envelope
            .sign(&signer, SignatureKind::evm_default())
            .await
            .unwrap();

        assert_eq!(
            signature.evm_signer_address(&envelope).unwrap(),
            signer.address()
        );

        // also see that we can recover with no prefix
        let signature = envelope
            .sign(
                &signer,
                SignatureKind {
                    algorithm: wavs_types::SignatureAlgorithm::Secp256k1,
                    prefix: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            signature.evm_signer_address(&envelope).unwrap(),
            signer.address()
        );

        // and that it fails if we try the wrong prefix
        let mut signature = envelope
            .sign(&signer, SignatureKind::evm_default())
            .await
            .unwrap();

        signature.kind.prefix = None;

        assert_ne!(
            signature.evm_signer_address(&envelope).unwrap(),
            signer.address()
        );

        // in both directions
        let mut signature = envelope
            .sign(
                &signer,
                SignatureKind {
                    algorithm: wavs_types::SignatureAlgorithm::Secp256k1,
                    prefix: None,
                },
            )
            .await
            .unwrap();

        signature.kind.prefix = Some(wavs_types::SignaturePrefix::Eip191);

        assert_ne!(
            signature.evm_signer_address(&envelope).unwrap(),
            signer.address()
        );
    }

    fn mock_signer() -> PrivateKeySigner {
        MnemonicBuilder::<English>::default()
            .word_count(24)
            .build_random()
            .unwrap()
    }

    fn mock_envelope() -> Envelope {
        Envelope {
            payload: Bytes::from_static(&[1, 2, 3]),
            eventId: FixedBytes([1; 20]),
            ordering: FixedBytes([0; 12]),
        }
    }

    #[tokio::test]
    async fn test_nonce_desync_with_multiple_transactions() {
        let test_deps = TestContractDeps::new().await;
        let primary_client = test_deps.client.clone();
        let primary_address = primary_client.address();

        let fast_nonce_manager = match &primary_client.nonce_manager {
            AnyNonceManager::Fast(manager) => manager.clone(),
            AnyNonceManager::Safe(_) => panic!("test requires FastNonceManager"),
        };

        // Ensure the in-memory counter is aligned with the chain before creating a gap.
        let initial_chain_nonce = fast_nonce_manager
            .set_current_nonce(&primary_client.provider)
            .await
            .expect("should sync initial nonce");
        assert_eq!(
            fast_nonce_manager.current_nonce(),
            initial_chain_nonce,
            "FastNonceManager must match chain nonce after initial sync"
        );

        // Create a secondary client using the same credentials to consume nonces directly from the chain.
        let secondary_client = EvmSigningClient::new(EvmSigningClientConfig::new(
            primary_client.config.endpoint.clone(),
            Credential::new(
                "test test test test test test test test test test test junk".to_string(),
            ),
        ))
        .await
        .expect("secondary client should initialize");

        // Send a burst of transactions that bypass the primary FastNonceManager.
        let transactions_to_send = 3;
        for i in 1..=transactions_to_send {
            tracing::info!("Secondary client submitting tx {i}");
            secondary_client
                .transfer_funds(secondary_client.signer.address(), "0.001")
                .await
                .expect("secondary transfer should succeed");
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Allow time for the transfers to be mined.
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Calculate the resulting chain nonce gap.
        let chain_nonce_after_gap = primary_client
            .provider
            .get_transaction_count(primary_address)
            .await
            .expect("should read nonce after gap");
        assert!(
            chain_nonce_after_gap
                >= initial_chain_nonce + u64::try_from(transactions_to_send).unwrap(),
            "expected the chain nonce to advance by at least the number of secondary transactions"
        );
        assert_eq!(
            fast_nonce_manager.current_nonce(),
            initial_chain_nonce,
            "FastNonceManager should still have the stale nonce before retries"
        );

        // Deploy minimal test contracts to exercise send_envelope_signatures end-to-end.
        let manager_address = *TestServiceManager::deploy(secondary_client.provider.clone())
            .await
            .expect("deploy manager")
            .address();
        let handler_address =
            *TestServiceHandler::deploy(secondary_client.provider.clone(), manager_address)
                .await
                .expect("deploy handler")
                .address();

        let chain_nonce_before_submission = primary_client
            .provider
            .get_transaction_count(primary_address)
            .await
            .expect("should read nonce prior to submission");
        assert!(
            chain_nonce_before_submission > chain_nonce_after_gap,
            "contract deployments should increase the chain nonce"
        );
        assert_eq!(
            fast_nonce_manager.current_nonce(),
            initial_chain_nonce,
            "FastNonceManager must still hold the stale nonce before attempting submission"
        );

        // Build a signed envelope referencing the primary signer.
        let envelope = mock_envelope();
        let signature = envelope
            .sign(primary_client.signer.as_ref(), SignatureKind::evm_default())
            .await
            .expect("signing envelope should succeed");
        let current_block = primary_client
            .provider
            .get_block_number()
            .await
            .expect("should get block height");
        let signature_data = envelope
            .signature_data(vec![signature], current_block.saturating_sub(1))
            .expect("signature data should build");

        // With a stale nonce cached, the first attempt will fail, triggering the retry path.
        let receipt = primary_client
            .send_envelope_signatures(
                envelope.clone(),
                signature_data.clone(),
                handler_address,
                None,
                None,
            )
            .await
            .expect("transaction should succeed after nonce refresh");
        assert!(
            receipt.status(),
            "receipt from handler submission should indicate success"
        );

        // The FastNonceManager must now reflect the refreshed chain nonce.
        let refreshed_counter = fast_nonce_manager.current_nonce();
        assert!(
            refreshed_counter > chain_nonce_before_submission,
            "FastNonceManager counter should advance past the stale value"
        );

        let post_submission_nonce = primary_client
            .provider
            .get_transaction_count(primary_address)
            .await
            .expect("should read nonce after submission");
        assert_eq!(
            post_submission_nonce,
            chain_nonce_before_submission + 1,
            "chain nonce should advance exactly once after a successful retry"
        );

        let submitted_tx = primary_client
            .provider
            .get_transaction_by_hash(receipt.transaction_hash)
            .await
            .expect("should fetch submitted transaction")
            .expect("transaction must exist on chain");
        assert_eq!(
            submitted_tx.inner.nonce(),
            chain_nonce_before_submission,
            "retried transaction must use the refreshed nonce"
        );

        tracing::info!("✓ send_envelope_signatures recovered from nonce desync");
    }
}
