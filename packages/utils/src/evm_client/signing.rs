use alloy_primitives::Address;
use alloy_rpc_types_eth::TransactionReceipt;
use alloy_signer::k256::SecretKey;
use alloy_signer_local::{coins_bip39::English, MnemonicBuilder, PrivateKeySigner};
use wavs_types::{Envelope, EnvelopeSignature, SignatureData};

use crate::error::EvmClientError;

use super::EvmSigningClient;

pub fn make_signer(credentials: &str, hd_index: Option<u32>) -> super::Result<PrivateKeySigner> {
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
            .phrase(credentials)
            .index(hd_index)?
            .build()?),
    }
}

impl EvmSigningClient {
    pub async fn send_envelope_signatures(
        &self,
        envelope: Envelope,
        signatures: Vec<EnvelopeSignature>,
        block_height: u64,
        service_handler: Address,
        max_gas: Option<u64>,
    ) -> Result<TransactionReceipt, EvmClientError> {
        let mut operators = Vec::with_capacity(signatures.len());

        for signature in &signatures {
            // TODO - no need for this... see if we can remove it
            // tracking issue: https://github.com/Lay3rLabs/wavs-middleware/issues/63
            operators.push(
                signature
                    .evm_signer_address(&envelope)
                    .map_err(EvmClientError::RecoverSignerAddress)?,
            );
        }

        let signature_data = SignatureData {
            operators,
            signatures: signatures
                .into_iter()
                .map(|s| s.as_bytes().into())
                .collect(),
            referenceBlock: block_height as u32,
        };

        let gas = match max_gas {
            None => {
                let gas_estimate = self
                    .service_handler(service_handler)
                    .handleSignedEnvelope(envelope.clone(), signature_data.clone())
                    .estimate_gas()
                    .await
                    .map_err(|e| EvmClientError::TransactionWithoutReceipt(e.into()))?;

                // pad it with a multiplier to account for gas fluctuations
                ((gas_estimate as f32) * self.gas_estimate_multiplier) as u64
            }

            Some(gas) => {
                // EIP-1559 has a default 30m gas limit per block without override. Else:
                // 'a intrinsic gas too high -- tx.gas_limit > env.block.gas_limit' is thrown
                gas.min(30_000_000)
            }
        };

        let receipt = self
            .service_handler(service_handler)
            .handleSignedEnvelope(envelope, signature_data)
            .gas(gas)
            .send()
            .await
            .map_err(|e| EvmClientError::TransactionWithoutReceipt(e.into()))?
            .get_receipt()
            .await
            .map_err(|e| EvmClientError::TransactionWithoutReceipt(e.into()))?;

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
    use wavs_types::{Envelope, EnvelopeExt};

    #[tokio::test]
    async fn signature_validation() {
        let signer = mock_signer();
        let envelope = mock_envelope();

        let signature = envelope.sign(&signer).await.unwrap();

        assert_eq!(
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
            eventId: FixedBytes([0; 20]),
            ordering: FixedBytes([0; 12]),
        }
    }
}
