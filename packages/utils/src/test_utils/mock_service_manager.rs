use alloy_sol_types::sol;
use anyhow::Result;
use bip39::Mnemonic;

use crate::{
    evm_client::{EvmSigningClient, NonceManagerKind},
    test_utils::middleware::{
        AvsOperator, MiddlewareServiceManagerAddresses, MiddlewareServiceManagerConfig,
        MiddlewareSetServiceUri,
    },
};

#[derive(Debug)]
pub struct MockServiceManager {
    pub client: EvmSigningClient,
    pub config: MiddlewareServiceManagerConfig,
    pub address: alloy_primitives::Address,
    pub all_addresses: MiddlewareServiceManagerAddresses,
}

// because the client will be used with the docker image
// and we can't control or even know how the nonce gets used
// we need to:
// 1. generate a random wallet and fund it from the wallet
// 2. use the safe nonce manager to avoid nonce errors
async fn generate_client(wallet_client: &EvmSigningClient) -> Result<EvmSigningClient> {
    let mut chain_config = wallet_client.config.clone();
    chain_config.credential = Mnemonic::generate(24).unwrap().to_string();
    chain_config.nonce_manager_kind = NonceManagerKind::Safe;

    let client = EvmSigningClient::new(chain_config).await?;
    wallet_client.transfer_funds(client.address(), "1").await?;

    Ok(client)
}

impl MockServiceManager {
    pub async fn deploy_middleware(
        config: MiddlewareServiceManagerConfig,
        wallet_client: EvmSigningClient,
    ) -> Result<Self> {
        let client = generate_client(&wallet_client).await?;

        let private_key_hex = const_hex::encode(client.signer.credential().to_bytes().to_vec());
        let rpc_url = client.config.endpoint.to_string();

        let all_addresses =
            MiddlewareServiceManagerAddresses::deploy(&config, &rpc_url, &private_key_hex).await?;

        Ok(Self {
            client,
            config,
            address: all_addresses.address,
            all_addresses,
        })
    }

    pub async fn set_service_uri(&self, uri: String) -> anyhow::Result<()> {
        MiddlewareSetServiceUri {
            rpc_url: self.client.config.endpoint.to_string(),
            service_manager_address: self.address,
            deployer_key_hex: const_hex::encode(
                self.client.signer.credential().to_bytes().to_vec(),
            ),
            service_uri: uri,
        }
        .run()
        .await
    }

    pub async fn set_operator_details(
        &self,
        AvsOperator {
            operator,
            signer,
            weight,
        }: AvsOperator,
    ) -> anyhow::Result<()> {
        sol! {
            #[sol(rpc)]
            interface StakeRegistry {
                function setOperatorDetails(
                    address operator,
                    address signingKeyAddress,
                    uint256 weight
                ) external;
            }
        }

        let instance = StakeRegistry::new(
            self.all_addresses.stake_registry_address,
            self.client.provider.clone(),
        );

        instance
            .setOperatorDetails(operator, signer, weight.try_into()?)
            .send()
            .await?
            .watch()
            .await?;

        Ok(())
    }
}
