use super::layer_service_handler::IWavsServiceHandler;
use super::IWavsServiceHandlerT;
use crate::eth_client::EthSigningClient;
use alloy::contract::Error;
use alloy::primitives::{Bytes, FixedBytes};
use alloy::{
    dyn_abi::DynSolValue,
    primitives::{eip191_hash_message, keccak256, Address, U256},
    providers::Provider,
    signers::SignerSync,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct ServiceHandlerClient {
    pub eth: EthSigningClient,
    pub address: Address,
    pub contract: IWavsServiceHandlerT,
}

impl ServiceHandlerClient {
    pub fn new(eth: EthSigningClient, address: Address) -> Self {
        let contract = IWavsServiceHandler::new(address, eth.provider.clone());

        Self {
            eth,
            address,
            contract,
        }
    }

    // helper to add a single signed payload to the contract
    pub async fn add_signed_payload(
        &self,
        signed_payload: SignedPayload,
        gas: Option<u64>,
    ) -> Result<()> {
        // EIP-1559 has a default 30m gas limit per block without override. Else:
        // 'a intrinsic gas too high -- tx.gas_limit > env.block.gas_limit' is thrown
        let gas = gas.unwrap_or(1_000_000).min(30_000_000);
        tracing::debug!("Adding signed payload with gas {}", gas);

        let (data, signature) = signed_payload.into_submission_abi();
        let result = self
            .contract
            .handleSignedData(data, signature)
            .gas(gas)
            .send()
            .await;

        match result {
            Ok(tx) => {
                let receipt = tx.get_receipt().await?;
                tracing::debug!("Transaction receipt: {:?}", receipt);
            }
            Err(e) => {
                tracing::error!("Failed to send signed payload with error: {:#}", e);
                match e {
                    Error::TransportError(ref e) => {
                        tracing::error!("Transport error: {}", e);
                    }
                    _ => {
                        tracing::error!("Other contract error: {:?}", e);
                    }
                }
                return Err(anyhow::anyhow!("Failed to send signed payload: {:#}", e));
            }
        }

        Ok(())
    }

    pub async fn sign_payload(&self, data: Vec<u8>) -> Result<SignedPayload> {
        let data_hash = eip191_hash_message(keccak256(&data));
        let signature = self.eth.signer.sign_hash_sync(&data_hash)?.into();

        Ok(SignedPayload {
            operator: self.eth.address(),
            data,
            data_hash,
            signature,
            signed_block_height: self.eth.provider.get_block_number().await? - 1,
        })
    }
}

// A single signed payload, meant to be passed around on the rust side
// i.e. gets sent to aggregator
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedPayload {
    pub operator: Address,
    pub data: Vec<u8>,
    pub data_hash: FixedBytes<32>,
    pub signature: Vec<u8>,
    pub signed_block_height: u64,
}

impl SignedPayload {
    pub fn into_submission_abi(self) -> (Bytes, Bytes) {
        let operators: Vec<DynSolValue> = vec![self.operator.into()];
        let signature: Vec<DynSolValue> = vec![DynSolValue::Bytes(self.signature)];
        let signed_block_height = U256::from(self.signed_block_height);

        let signature = DynSolValue::Tuple(vec![
            DynSolValue::Array(operators),
            DynSolValue::Array(signature),
            DynSolValue::Uint(signed_block_height, 64),
        ])
        .abi_encode_params();

        (self.data.into(), signature.into())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SignedData {
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
}
