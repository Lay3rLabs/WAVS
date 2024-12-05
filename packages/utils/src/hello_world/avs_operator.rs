use alloy::primitives::{Address, FixedBytes, TxHash, U256};
use anyhow::Context;
use chrono::Utc;
use rand::RngCore;

use crate::{
    eigen_client::solidity_types::misc::AVSDirectory,
    hello_world::solidity_types::stake_registry::{
        ECDSAStakeRegistry, ISignatureUtils::SignatureWithSaltAndExpiry,
    },
};
use alloy::signers::SignerSync;

use super::HelloWorldFullClient;

use anyhow::Result;

impl HelloWorldFullClient {
    // TODO: move to a core impl
    pub async fn calculate_operator_avs_registration_digest_hash(
        &self,
        operator: Address,
        avs: Address,
        salt: FixedBytes<32>,
        expiry: U256,
    ) -> Result<FixedBytes<32>> {
        let contract_avs_directory =
            AVSDirectory::new(self.core.avs_directory, self.eth.http_provider.clone());

        let operator_avs_registration_digest_hash = contract_avs_directory
            .calculateOperatorAVSRegistrationDigestHash(operator, avs, salt, expiry)
            .call()
            .await
            .context("AlloyContractError")?;

        let AVSDirectory::calculateOperatorAVSRegistrationDigestHashReturn { _0: avs_hash } =
            operator_avs_registration_digest_hash;

        Ok(avs_hash)
    }

    pub async fn register_operator(&self) -> Result<TxHash> {
        let mut salt = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut salt);

        let salt = FixedBytes::from_slice(&salt);
        let now = Utc::now().timestamp();
        let expiry: U256 = U256::from(now + 3600);

        let digest_hash = self
            .calculate_operator_avs_registration_digest_hash(
                self.eth.address(),
                self.hello_world.hello_world_service_manager,
                salt,
                expiry,
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
