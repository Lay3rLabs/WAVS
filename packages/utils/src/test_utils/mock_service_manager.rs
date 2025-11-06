use alloy_primitives::Address;
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;

use wavs_types::IWavsServiceManager::IWavsServiceManagerInstance;

use crate::{
    evm_client::EvmSigningClient,
    test_utils::middleware::evm::{
        EvmMiddleware, EvmMiddlewareServiceManager, MiddlewareServiceManagerConfig,
    },
};

pub struct MockEvmServiceManager {
    #[allow(dead_code)]
    deployer: LocalSigner<SigningKey>,
    middleware_instance: EvmMiddleware,
    service_manager: EvmMiddlewareServiceManager,
    client: EvmSigningClient,
}

impl MockEvmServiceManager {
    // because the client will be used with the docker image
    // and we can't control or even know how the nonce gets used
    // we need to generate a random key and fund it from the wallet
    // otherwise we may try to run transactions in parallel with the same nonce
    pub async fn new(
        middleware_instance: EvmMiddleware,
        wallet_client: EvmSigningClient,
    ) -> Result<Self> {
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: Duration = Duration::from_millis(1000);

        for attempt in 0..MAX_RETRIES {
            let deployer = MnemonicBuilder::<English>::default()
                .word_count(24)
                .build_random()?;

            tracing::info!(
                "Starting transfer_funds for deployer: {:?} (attempt {})",
                deployer.address(),
                attempt + 1
            );
            wallet_client
                .transfer_funds(deployer.address(), "1")
                .await?;
            tracing::info!(
                "Completed transfer_funds for deployer: {:?}",
                deployer.address()
            );

            let deployer_key_hex = const_hex::encode(deployer.credential().to_bytes());
            let rpc_url = wallet_client.config.endpoint.to_string();

            tracing::info!(
                "Starting deploy_service_manager for deployer: {:?} (attempt {})",
                deployer.address(),
                attempt + 1
            );

            match middleware_instance
                .deploy_service_manager(rpc_url.clone(), Some(deployer_key_hex.clone()))
                .await
            {
                Ok(service_manager) => {
                    tracing::info!(
                        "Completed deploy_service_manager for deployer: {:?}, address: {}",
                        deployer.address(),
                        service_manager.address
                    );
                    return Ok(Self {
                        deployer,
                        service_manager,
                        middleware_instance,
                        client: wallet_client,
                    });
                }
                Err(e) => {
                    if attempt < MAX_RETRIES - 1 {
                        tracing::warn!(
                            "Service manager deployment attempt {} failed: {:?}. Retrying in {:?}...",
                            attempt + 1, e, RETRY_DELAY
                        );
                        sleep(RETRY_DELAY).await;
                        continue;
                    } else {
                        return Err(anyhow::anyhow!(
                            "Service manager deployment failed after {} attempts: {:?}",
                            MAX_RETRIES,
                            e
                        ));
                    }
                }
            }
        }

        unreachable!()
    }

    pub fn address(&self) -> Address {
        self.service_manager.address
    }

    pub async fn set_service_uri(&self, uri: String) -> anyhow::Result<()> {
        self.middleware_instance
            .set_service_manager_uri(&self.service_manager, &uri)
            .await?;

        Ok(())
    }

    pub async fn configure(&self, config: &MiddlewareServiceManagerConfig) -> anyhow::Result<()> {
        self.middleware_instance
            .configure_service_manager(&self.service_manager, config)
            .await
    }

    /// Validate that operators are properly registered in the service manager
    pub async fn validate_operator_registration(
        &self,
        config: &MiddlewareServiceManagerConfig,
    ) -> anyhow::Result<()> {
        const MAX_RETRIES: usize = 30;
        const RETRY_DELAY: Duration = Duration::from_millis(500);

        for attempt in 0..MAX_RETRIES {
            let mut all_valid = true;

            for avs_operator in &config.avs_operators {
                // Check if the signer-to-operator mapping is correctly registered
                let registered_operator = self
                    .get_latest_operator_for_signing_key(avs_operator.signer)
                    .await?;
                if registered_operator != avs_operator.operator {
                    if attempt < MAX_RETRIES - 1 {
                        tracing::debug!(
                            "Attempt {}: Expected operator {} for signer {}, got {}. Retrying...",
                            attempt + 1,
                            avs_operator.operator,
                            avs_operator.signer,
                            registered_operator
                        );
                        all_valid = false;
                        break;
                    } else {
                        return Err(anyhow::anyhow!(
                            "Operator registration failed: Expected operator {} for signer {}, got {}",
                            avs_operator.operator, avs_operator.signer, registered_operator
                        ));
                    }
                }

                // Check if the operator has weight
                let weight = self.get_operator_weight(avs_operator.operator).await?;
                if weight == 0 {
                    if attempt < MAX_RETRIES - 1 {
                        tracing::debug!(
                            "Attempt {}: Operator {} has 0 weight. Retrying...",
                            attempt + 1,
                            avs_operator.operator
                        );
                        all_valid = false;
                        break;
                    } else {
                        return Err(anyhow::anyhow!(
                            "Operator {} has 0 weight after configuration",
                            avs_operator.operator
                        ));
                    }
                }
            }

            if all_valid {
                tracing::debug!(
                    "Operator registration validated successfully on attempt {}",
                    attempt + 1
                );
                return Ok(());
            }

            sleep(RETRY_DELAY).await;
        }

        Err(anyhow::anyhow!(
            "Operator registration validation failed after {} attempts",
            MAX_RETRIES
        ))
    }

    /// Get the latest operator for a given signing key with retry logic
    async fn get_latest_operator_for_signing_key(
        &self,
        signing_key: Address,
    ) -> anyhow::Result<Address> {
        const VALIDATION_RETRIES: usize = 10;
        const VALIDATION_RETRY_DELAY: Duration = Duration::from_millis(200);

        for attempt in 0..VALIDATION_RETRIES {
            let service_manager = IWavsServiceManagerInstance::new(
                self.service_manager.address,
                &self.client.provider,
            );

            match service_manager
                .getLatestOperatorForSigningKey(signing_key)
                .call()
                .await
            {
                Ok(result) => return Ok(Address::from(result.0)),
                Err(e) => {
                    if attempt < VALIDATION_RETRIES - 1 {
                        tracing::debug!(
                            "getLatestOperatorForSigningKey attempt {} failed: {:?}. Retrying...",
                            attempt + 1,
                            e
                        );
                        sleep(VALIDATION_RETRY_DELAY).await;
                        continue;
                    } else {
                        return Err(anyhow::anyhow!(
                            "getLatestOperatorForSigningKey failed after {} attempts: {:?}",
                            VALIDATION_RETRIES,
                            e
                        ));
                    }
                }
            }
        }

        unreachable!()
    }

    /// Get the operator weight with retry logic
    async fn get_operator_weight(&self, operator: Address) -> anyhow::Result<u64> {
        const VALIDATION_RETRIES: usize = 10;
        const VALIDATION_RETRY_DELAY: Duration = Duration::from_millis(200);

        for attempt in 0..VALIDATION_RETRIES {
            let service_manager = IWavsServiceManagerInstance::new(
                self.service_manager.address,
                &self.client.provider,
            );

            match service_manager.getOperatorWeight(operator).call().await {
                Ok(result) => {
                    // Convert U256 to u64 safely
                    let weight = result
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Operator weight too large for u64"))?;
                    return Ok(weight);
                }
                Err(e) => {
                    if attempt < VALIDATION_RETRIES - 1 {
                        tracing::debug!(
                            "getOperatorWeight attempt {} failed: {:?}. Retrying...",
                            attempt + 1,
                            e
                        );
                        sleep(VALIDATION_RETRY_DELAY).await;
                        continue;
                    } else {
                        return Err(anyhow::anyhow!(
                            "getOperatorWeight failed after {} attempts: {:?}",
                            VALIDATION_RETRIES,
                            e
                        ));
                    }
                }
            }
        }

        unreachable!()
    }
}
