use alloy::{
    primitives::{eip191_hash_message, keccak256, Address},
    rpc::types::TransactionReceipt,
    signers::Signer,
    sol_types::SolValue,
};
use wavs_types::{Envelope, SignatureData, SignerAddress};

use crate::error::EthClientError;

use super::EthSigningClient;

type SignerAndSignature = (SignerAddress, Vec<u8>);

impl EthSigningClient {
    pub async fn sign_envelope(&self, envelope: &Envelope) -> Result<Vec<u8>, EthClientError> {
        let envelope_bytes = envelope.abi_encode();
        let envelope_hash = eip191_hash_message(keccak256(&envelope_bytes));
        Ok(self
            .signer
            .sign_hash(&envelope_hash)
            .await
            .map_err(|e| EthClientError::Signing(e.into()))?
            .into())
    }

    pub async fn send_envelope_signatures(
        &self,
        envelope: Envelope,
        signer_and_signatures: Vec<SignerAndSignature>,
        block_height: u64,
        service_handler: Address,
        max_gas: Option<u64>,
    ) -> Result<TransactionReceipt, EthClientError> {
        let mut operators = Vec::with_capacity(signer_and_signatures.len());
        let mut signatures = Vec::with_capacity(signer_and_signatures.len());

        for (signer, signature) in signer_and_signatures.into_iter() {
            operators.push(signer.eth_unchecked());
            signatures.push(signature.into());
        }

        let signature_data = SignatureData {
            operators,
            signatures,
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
