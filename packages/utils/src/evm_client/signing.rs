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
                                // Continue with the same tx_builder - it will now use the refreshed nonce
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
    use alloy_primitives::FixedBytes;
    use alloy_signer_local::{coins_bip39::English, MnemonicBuilder, PrivateKeySigner};
    use wavs_types::{Envelope, EnvelopeSigner, SignatureKind};

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
            payload: vec![1, 2, 3].into(),
            eventId: FixedBytes([1; 20]),
            ordering: FixedBytes([0; 12]),
        }
    }
}
