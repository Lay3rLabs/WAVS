use std::process::Stdio;
use std::time::Duration;

use alloy_primitives::Address;
use anyhow::{bail, Result};
use serde::Deserialize;
use tokio::process::Command;

use super::{EvmMiddlewareServiceManager, MiddlewareServiceManagerConfig};

/// PoaBlsMiddleware deploys and configures BLS poa-middleware contracts using
/// local `forge` and `cast` commands directly from the `contracts/poa-middleware/`
/// submodule. Unlike `PoaMiddleware` which uses a Docker image, this middleware
/// avoids Docker entirely and calls forge/cast from the host.
#[derive(Default)]
pub struct PoaBlsMiddleware {}

impl PoaBlsMiddleware {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve the path to the `contracts/poa-middleware/` submodule.
    ///
    /// `workspace_path()` returns the WAVS crate root (e.g. `/path/to/WAVS`).
    /// The monorepo is one level up, so `contracts/poa-middleware/` is at
    /// `workspace_path().parent()/contracts/poa-middleware/`.
    fn resolve_poa_middleware_path() -> Result<std::path::PathBuf> {
        let workspace = crate::filesystem::workspace_path();
        let monorepo_root = workspace.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot find monorepo root from workspace path: {}",
                workspace.display()
            )
        })?;
        let poa_path = monorepo_root.join("contracts").join("poa-middleware");
        if !poa_path.exists() {
            bail!(
                "poa-middleware submodule not found at {}. \
                 Ensure contracts/poa-middleware/ submodule is checked out.",
                poa_path.display()
            );
        }
        Ok(poa_path)
    }

    pub async fn deploy_service_manager(
        &self,
        rpc_url: String,
        deployer_key_hex: String,
    ) -> Result<EvmMiddlewareServiceManager> {
        let poa_middleware_dir = Self::resolve_poa_middleware_path()?;

        tracing::info!(
            "Building BLS contracts from poa-middleware submodule at {}",
            poa_middleware_dir.display()
        );

        // Build BLS contracts (FOUNDRY_PROFILE=bls forge build)
        let build_status = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("forge")
                .arg("build")
                .env("FOUNDRY_PROFILE", "bls")
                .current_dir(&poa_middleware_dir)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait(),
        )
        .await??;

        if !build_status.success() {
            bail!("Failed to build BLS contracts from poa-middleware submodule");
        }

        tracing::info!("Deploying BLS POA middleware contracts via forge script");

        // Deploy BLS contracts via forge script
        // FOUNDRY_PROFILE=bls forge script contracts/script/bls/POAMiddlewareDeployer.s.sol \
        //   --rpc-url $RPC_URL --private-key $KEY -vvv --broadcast --skip-simulation
        let deploy_output = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("forge")
                .args([
                    "script",
                    "contracts/script/bls/POAMiddlewareDeployer.s.sol",
                    "--rpc-url",
                    &rpc_url,
                    "--private-key",
                    &deployer_key_hex,
                    "-vvv",
                    "--broadcast",
                    "--skip-simulation",
                ])
                .env("FOUNDRY_PROFILE", "bls")
                .current_dir(&poa_middleware_dir)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait(),
        )
        .await??;

        if !deploy_output.success() {
            bail!("Failed to deploy BLS POA middleware contracts via forge script");
        }

        // Parse deployment JSON from poa-middleware/deployments/poa-bls/poa_deploy.json
        let deploy_json_path = poa_middleware_dir.join("deployments/poa-bls/poa_deploy.json");
        let deploy_json = tokio::fs::read_to_string(&deploy_json_path)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read BLS deployment JSON at {}: {}. \
                 Ensure forge script completed successfully.",
                    deploy_json_path.display(),
                    e
                )
            })?;

        #[derive(Deserialize)]
        struct PoaDeploymentJson {
            addresses: PoaAddresses,
        }

        #[derive(Deserialize)]
        struct PoaAddresses {
            #[serde(rename = "POAStakeRegistry")]
            poa_stake_registry: Address,
            #[serde(rename = "proxyAdmin")]
            proxy_admin: Address,
        }

        let deployment: PoaDeploymentJson = serde_json::from_str(&deploy_json)
            .map_err(|e| anyhow::anyhow!("Failed to parse BLS deployment JSON: {}", e))?;

        let poa_address = deployment.addresses.poa_stake_registry;

        tracing::info!(
            "BLS POA middleware deployed: stake_registry={}",
            poa_address
        );

        Ok(EvmMiddlewareServiceManager {
            deployer_key_hex,
            rpc_url,
            id: format!("bls-poa-{}", poa_address),
            container_id: None, // No Docker container -- local forge deployment
            address: poa_address,
            proxy_admin: deployment.addresses.proxy_admin,
            impl_address: poa_address,
            stake_registry_address: poa_address,
            stake_registry_impl_address: poa_address,
        })
    }

    pub async fn configure_service_manager(
        &self,
        service_manager: &EvmMiddlewareServiceManager,
        config: &MiddlewareServiceManagerConfig,
    ) -> Result<()> {
        for i in 0..config.operators.len() {
            let operator = &config.operators[i];
            let weight = &config.weights[i];
            let avs_operator = &config.avs_operators[i];

            // Step 1: Register operator via cast send
            tracing::info!(
                "Registering BLS operator {} with weight {}",
                operator,
                weight
            );
            let status = tokio::time::timeout(
                Self::DEFAULT_TIMEOUT,
                Command::new("cast")
                    .args([
                        "send",
                        &format!("{}", service_manager.address),
                        "registerOperator(address,uint256)",
                        &format!("{:?}", operator),
                        &weight.to_string(),
                        "--private-key",
                        &service_manager.deployer_key_hex,
                        "--rpc-url",
                        &service_manager.rpc_url,
                    ])
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?
                    .wait(),
            )
            .await??;

            if !status.success() {
                bail!("Failed to register BLS operator {}", operator);
            }

            // Step 2: Update BLS signing key (operator signs with their own key)
            let operator_key = avs_operator.operator_private_key.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Operator private key required for BLS middleware")
            })?;

            let bls_pubkey = avs_operator
                .bls_pubkey
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("BLS pubkey required for BLS middleware"))?;

            let bls_proof = avs_operator
                .bls_proof
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("BLS proof required for BLS middleware"))?;

            let bls_pubkey_hex = format!("0x{}", const_hex::encode(bls_pubkey));
            let bls_proof_hex = format!("0x{}", const_hex::encode(bls_proof));

            // Fund the operator address via anvil_setBalance so it can pay for gas.
            // The operator key is derived from the test mnemonic and may start with 0 ETH.
            let operator_addr_hex = format!("{:?}", operator);
            let fund_status = tokio::time::timeout(
                Self::DEFAULT_TIMEOUT,
                Command::new("cast")
                    .args([
                        "rpc",
                        "anvil_setBalance",
                        &operator_addr_hex,
                        "0x10000000000000000000000",
                        "--rpc-url",
                        &service_manager.rpc_url,
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?
                    .wait(),
            )
            .await??;
            if !fund_status.success() {
                bail!(
                    "Failed to fund operator address {} for BLS key update",
                    operator
                );
            }

            tracing::info!(
                "Updating BLS signing key for operator {} (pubkey {} bytes, proof {} bytes)",
                operator,
                bls_pubkey.len(),
                bls_proof.len()
            );

            let status = tokio::time::timeout(
                Self::DEFAULT_TIMEOUT,
                Command::new("cast")
                    .args([
                        "send",
                        &format!("{}", service_manager.address),
                        "updateOperatorSigningKey(bytes,bytes)",
                        &bls_pubkey_hex,
                        &bls_proof_hex,
                        "--private-key",
                        operator_key,
                        "--rpc-url",
                        &service_manager.rpc_url,
                    ])
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?
                    .wait(),
            )
            .await??;

            if !status.success() {
                bail!("Failed to update BLS signing key for operator {}", operator);
            }
        }

        // Step 3: Update quorum
        tracing::info!(
            "Updating BLS quorum: {}/{}",
            config.quorum_numerator,
            config.quorum_denominator
        );

        let status = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("cast")
                .args([
                    "send",
                    &format!("{}", service_manager.address),
                    "updateQuorum(uint256,uint256)",
                    &config.quorum_numerator.to_string(),
                    &config.quorum_denominator.to_string(),
                    "--private-key",
                    &service_manager.deployer_key_hex,
                    "--rpc-url",
                    &service_manager.rpc_url,
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait(),
        )
        .await??;

        if !status.success() {
            bail!("Failed to update BLS quorum");
        }

        Ok(())
    }

    pub async fn set_service_manager_uri(
        &self,
        service_manager: &EvmMiddlewareServiceManager,
        service_uri: &str,
    ) -> Result<()> {
        tracing::debug!(
            "Setting service URI for BLS POA: address={}, uri='{}'",
            service_manager.address,
            service_uri
        );

        let status = tokio::time::timeout(
            Self::DEFAULT_TIMEOUT,
            Command::new("cast")
                .args([
                    "send",
                    &format!("{}", service_manager.address),
                    "setServiceURI(string)",
                    service_uri,
                    "--private-key",
                    &service_manager.deployer_key_hex,
                    "--rpc-url",
                    &service_manager.rpc_url,
                ])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?
                .wait(),
        )
        .await??;

        if !status.success() {
            bail!(
                "Failed to set BLS service URI for {}",
                service_manager.address
            );
        }

        Ok(())
    }
}
