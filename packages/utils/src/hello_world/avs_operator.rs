use alloy::primitives::{FixedBytes, TxHash, U256};
use chrono::Utc;
use rand::RngCore;

use crate::hello_world::solidity_types::stake_registry::{
    ECDSAStakeRegistry, ISignatureUtils::SignatureWithSaltAndExpiry,
};
use alloy::signers::SignerSync;

use super::HelloWorldFullClient;

use anyhow::Result;

impl HelloWorldFullClient {
    pub async fn register_operator(&self) -> Result<TxHash> {
        let mut salt = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut salt);

        let salt = FixedBytes::from_slice(&salt);
        let now = Utc::now().timestamp();
        let expiry: U256 = U256::from(now + 3600);

        let digest_hash = self
            .core
            .calculate_operator_avs_registration_digest_hash(
                self.eth.address(),
                self.hello_world.hello_world_service_manager,
                salt,
                expiry,
                self.eth.http_provider.clone(),
            )
            .await?;

        let signature = self.eth.signer.sign_hash_sync(&digest_hash)?;
        let operator_signature = SignatureWithSaltAndExpiry {
            signature: signature.as_bytes().into(),
            salt,
            expiry,
        };
        let contract_ecdsa_stake_registry = ECDSAStakeRegistry::new(
            self.hello_world.stake_registry,
            self.eth.http_provider.clone(),
        );

        let register_hello_world_hash = contract_ecdsa_stake_registry
            .registerOperatorWithSignature(operator_signature, self.eth.signer.clone().address())
            .gas(500000)
            .send()
            .await?
            .get_receipt()
            .await?
            .transaction_hash;

        tracing::debug!(
            "Operator registered on AVS successfully :{} , tx_hash :{}",
            self.eth.signer.address(),
            register_hello_world_hash
        );
        Ok(register_hello_world_hash)
    }
}
