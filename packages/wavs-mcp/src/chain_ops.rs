use alloy_primitives::{keccak256, Address, Bytes, U256};
use alloy_signer::Signer;
use alloy_sol_macro::sol;
use alloy_sol_types::SolValue;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{process::Stdio, time::Duration};
use tempfile::TempDir;
use tokio::{fs, process::Command};
use utils::evm_client::{signing::make_signer, EvmEndpoint, EvmSigningClient, EvmSigningClientConfig};
use wavs_types::{Credential, ServiceManager};

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug)]
    SimpleServiceManager,
    "./abi/SimpleServiceManager.json"
}

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IPOAStakeRegistry {
        function registerOperator(address operator, uint256 weight) external;
        function updateOperatorSigningKey(address newSigningKey, bytes signingKeySignature) external;
    }
}

use IPOAStakeRegistry::IPOAStakeRegistryInstance;

const POA_MIDDLEWARE_IMAGE: &str = "ghcr.io/lay3rlabs/poa-middleware:1.0.1";
const DEPLOY_TIMEOUT_SECS: u64 = 120;
const POLL_INTERVAL_MS: u64 = 500;
const POA_DEPLOY_FILE: &str = "poa_deploy.json";

#[derive(Deserialize)]
struct PoaDeployJson {
    addresses: PoaAddresses,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PoaAddresses {
    #[serde(rename = "POAStakeRegistry")]
    poa_stake_registry: String,
}

fn parse_endpoint(rpc_url: &str) -> Result<EvmEndpoint> {
    rpc_url.parse::<EvmEndpoint>().context("invalid rpc_url")
}

/// Deploy a SimpleServiceManager contract. Returns `(address, tx_hash)`.
pub async fn deploy_service_manager(
    credential: &Credential,
    rpc_url: &str,
) -> Result<(String, String)> {
    let config = EvmSigningClientConfig::new(parse_endpoint(rpc_url)?, credential.clone());
    let client = EvmSigningClient::new(config).await?;

    let pending = SimpleServiceManager::deploy_builder(&client.provider)
        .send()
        .await
        .context("deploy SimpleServiceManager")?;
    let receipt = pending.get_receipt().await?;

    let address = receipt
        .contract_address
        .ok_or_else(|| anyhow::anyhow!("deploy tx had no contract address in receipt"))?;
    let tx_hash = receipt.transaction_hash;

    Ok((format!("{address}"), format!("{tx_hash}")))
}

/// Deploy the full POA middleware (POAStakeRegistry + proxy) via Docker.
/// Returns the proxy address.
pub async fn deploy_poa_service_manager(
    credential: &Credential,
    rpc_url: &str,
) -> Result<String> {
    let signer = make_signer(credential, None)?;
    let pk_bytes = signer.credential().to_bytes();
    let pk_hex = const_hex::encode(pk_bytes);

    let nodes_dir =
        TempDir::new().map_err(|e| anyhow::anyhow!("failed to create temp dir: {e}"))?;

    let output = Command::new("docker")
        .args([
            "run",
            "-d",
            "--network",
            "host",
            "--entrypoint",
            "",
            "-v",
            &format!("{}:/root/.nodes", nodes_dir.path().display()),
            POA_MIDDLEWARE_IMAGE,
            "tail",
            "-f",
            "/dev/null",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start Docker container")?
        .wait_with_output()
        .await
        .context("wait for Docker container")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("failed to start POA middleware container: {}", stderr);
    }

    let container_id = String::from_utf8(output.stdout)
        .context("read container ID")?
        .trim()
        .to_string();

    let result = tokio::time::timeout(
        Duration::from_secs(DEPLOY_TIMEOUT_SECS),
        async {
            let res = Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    &format!("FUNDED_KEY={pk_hex}"),
                    "-e",
                    &format!("RPC_URL={rpc_url}"),
                    "-e",
                    "DEPLOY_ENV=LOCAL",
                    &container_id,
                    "/wavs/scripts/cli.sh",
                    "deploy",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .context("spawn deploy script")?
                .wait()
                .await
                .context("deploy script")?;

            if !res.success() {
                anyhow::bail!("POA middleware deploy script exited with error");
            }

            let deploy_json_path = nodes_dir.path().join(POA_DEPLOY_FILE);
            loop {
                if deploy_json_path.exists() {
                    let content = fs::read_to_string(&deploy_json_path)
                        .await
                        .context("read deploy JSON")?;
                    return Ok::<String, anyhow::Error>(content);
                }
                tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
            }
        },
    )
    .await;

    let _ = Command::new("docker")
        .args(["stop", &container_id])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok();

    let json_content = result
        .map_err(|_| anyhow::anyhow!("POA deployment timed out after {DEPLOY_TIMEOUT_SECS}s"))??;

    let deploy: PoaDeployJson =
        serde_json::from_str(&json_content).context("parse deployment JSON")?;

    Ok(deploy.addresses.poa_stake_registry)
}

/// Derive the EVM address for the WAVS signing key (HD index 0).
/// Does not require network access.
pub fn get_signing_address(credential: &Credential) -> Result<String> {
    let signer = make_signer(credential, Some(0))?;
    Ok(format!("{}", signer.address()))
}

/// Register the signing key as an operator on a POAStakeRegistry contract.
/// `owner_credential` calls `registerOperator`; `signing_mnemonic` calls `updateOperatorSigningKey`.
/// Returns `(operator_address, register_tx, signing_key_tx)`.
pub async fn register_operator(
    service_manager: &ServiceManager,
    owner_credential: &Credential,
    signing_mnemonic: &Credential,
    weight: u64,
    rpc_url: &str,
) -> Result<(String, String, String)> {
    let endpoint = parse_endpoint(rpc_url)?;

    let contract_address: Address = match service_manager {
        ServiceManager::Evm { address, .. } => *address,
        _ => anyhow::bail!("only EVM service managers are supported"),
    };

    let owner_config = EvmSigningClientConfig::new(endpoint.clone(), owner_credential.clone());
    let owner_client = EvmSigningClient::new(owner_config).await?;

    let signing_config =
        EvmSigningClientConfig::new(endpoint, signing_mnemonic.clone()).with_hd_index(0);
    let signing_client = EvmSigningClient::new(signing_config).await?;
    let signing_key_addr: Address = signing_client.address();

    let registry_as_owner =
        IPOAStakeRegistryInstance::new(contract_address, owner_client.provider.clone());
    let register_receipt = registry_as_owner
        .registerOperator(signing_key_addr, U256::from(weight))
        .send()
        .await?
        .get_receipt()
        .await?;
    let register_tx = format!("{}", register_receipt.transaction_hash);

    let encoded: Vec<u8> = (signing_key_addr,).abi_encode();
    let message_hash = keccak256(&encoded);
    let sig = signing_client
        .signer
        .sign_hash(&message_hash)
        .await
        .context("sign message hash")?;
    let sig_bytes = Bytes::copy_from_slice(sig.as_bytes().as_ref());

    let registry_as_operator =
        IPOAStakeRegistryInstance::new(contract_address, signing_client.provider.clone());
    let signing_key_receipt = registry_as_operator
        .updateOperatorSigningKey(signing_key_addr, sig_bytes)
        .send()
        .await?
        .get_receipt()
        .await?;
    let signing_key_tx = format!("{}", signing_key_receipt.transaction_hash);

    Ok((format!("{signing_key_addr}"), register_tx, signing_key_tx))
}

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IServiceManagerSetUri {
        function setServiceURI(string uri) external;
    }
}

use IServiceManagerSetUri::IServiceManagerSetUriInstance;

/// Call `setServiceURI` on an IWavsServiceManager contract.
pub async fn set_service_uri(
    service_manager: &ServiceManager,
    credential: &Credential,
    rpc_url: &str,
    uri: String,
) -> Result<()> {
    let address: Address = match service_manager {
        ServiceManager::Evm { address, .. } => *address,
        _ => anyhow::bail!("only EVM service managers are supported"),
    };

    let config = EvmSigningClientConfig::new(parse_endpoint(rpc_url)?, credential.clone());
    let client = EvmSigningClient::new(config).await?;

    let contract = IServiceManagerSetUriInstance::new(address, client.provider.clone());
    contract.setServiceURI(uri).send().await?.watch().await?;

    Ok(())
}
