use alloy::{primitives::Address, rpc::types::TransactionReceipt, signers::Signer};
use wavs_types::{Envelope, EnvelopeExt, EnvelopeSignature, SignatureData};

use crate::error::EthClientError;

use super::EthSigningClient;

impl EthSigningClient {
    pub async fn sign_envelope(
        &self,
        envelope: &Envelope,
    ) -> Result<EnvelopeSignature, EthClientError> {
        let sig = self
            .signer
            .sign_hash(&envelope.eip191_hash())
            .await
            .map_err(|e| EthClientError::Signing(e.into()))?;

        Ok(EnvelopeSignature::Secp256k1(sig))
    }

    pub async fn send_envelope_signatures(
        &self,
        envelope: Envelope,
        signatures: Vec<EnvelopeSignature>,
        block_height: u64,
        service_handler: Address,
        max_gas: Option<u64>,
    ) -> Result<TransactionReceipt, EthClientError> {
        let mut operators = Vec::with_capacity(signatures.len());

        for signature in &signatures {
            // TODO - no need for this... see if we can remove it
            // tracking issue: https://github.com/Lay3rLabs/wavs-middleware/issues/63
            operators.push(
                signature
                    .eth_signer_address(&envelope)
                    .map_err(EthClientError::RecoverSignerAddress)?,
            );
        }

        let signature_data = SignatureData {
            operators,
            signatures: signatures
                .into_iter()
                .map(|s| s.into_vec().into())
                .collect(),
            referenceBlock: block_height as u32,
        };

        let gas = match max_gas {
            None => self
                .service_handler(service_handler)
                .handleSignedEnvelope(envelope.clone(), signature_data.clone())
                .estimate_gas()
                .await
                .map_err(|e| EthClientError::TransactionWithoutReceipt(e.into()))?,
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
            .map_err(|e| EthClientError::TransactionWithoutReceipt(e.into()))?
            .get_receipt()
            .await
            .map_err(|e| EthClientError::TransactionWithoutReceipt(e.into()))?;

        match receipt.status() {
            true => Ok(receipt),
            false => Err(EthClientError::TransactionWithReceipt(Box::new(receipt))),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy::{
        primitives::FixedBytes,
        signers::{
            k256::ecdsa::SigningKey,
            local::{coins_bip39::English, LocalSigner, MnemonicBuilder},
            SignerSync,
        },
    };
    use wavs_types::Envelope;

    #[test]
    fn signature_validation() {
        let signer = mock_signer();
        let envelope = mock_envelope();

        let signature = signer.sign_hash_sync(&envelope.eip191_hash()).unwrap();

        assert_eq!(
            signature
                .recover_address_from_prehash(&envelope.eip191_hash())
                .unwrap(),
            signer.address()
        );
    }

    fn mock_signer() -> LocalSigner<SigningKey> {
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
