use std::sync::Arc;

use rmcp::{
    ServerHandler,
    handler::server::tool::schema_for_type,
    model::*,
    schemars,
    service::{RequestContext, RoleServer},
};
use serde::Deserialize;

use crate::chain_ops;
use crate::client::WavsClient;
use crate::scaffold;

// ── Parameter structs ──────────────────────────────────────────────────────

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GetServiceParams {
    /// Chain key, e.g. "evm:31337" or "cosmos:mychain"
    pub chain: String,
    /// Service manager contract address
    pub address: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ServiceManagerParams {
    /// ServiceManager as a JSON object.
    /// EVM:    `{"evm":{"chain":"evm:31337","address":"0xAbCd..."}}`
    /// Cosmos: `{"cosmos":{"chain":"cosmos:mychain","address":"cosmos1..."}}`
    pub service_manager_json: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UploadComponentParams {
    /// Absolute path to the compiled `.wasm` file
    pub file_path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SimulateTriggerParams {
    /// Service ID — 64-char hex string derived from the ServiceManager
    pub service_id: String,
    /// Workflow ID — lowercase alphanumeric, 3–36 chars (e.g. "default")
    pub workflow_id: String,
    /// Trigger definition as JSON, e.g. `{"cron":{"schedule":"* * * * * * *","start_time":null,"end_time":null}}`
    /// Note: cron schedule uses 7-field format: `sec min hour dom month dow year`
    pub trigger_json: String,
    /// TriggerData as JSON, e.g. `{"Cron":{"trigger_time":0}}`
    pub data_json: String,
    /// How many times to fire the trigger (default: 1)
    pub count: Option<usize>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SaveServiceParams {
    /// Full Service definition as a JSON string.
    /// Must include: name, status, manager (evm/cosmos), workflows (map of workflow_id → {trigger, component, submit}).
    /// Requires dev endpoints enabled in wavs.toml.
    pub service_json: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DeployDevServiceParams {
    /// Full Service definition as a JSON string.
    /// Must include: name, status, manager (evm/cosmos), workflows (map of workflow_id → {trigger, component, submit}).
    /// Requires dev endpoints enabled in wavs.toml and --token.
    pub service_json: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct QueryKvParams {
    /// Service ID — 64-char hex string
    pub service_id: String,
    /// KV bucket name (as passed to `store::open` in the component)
    pub bucket: String,
    /// Key within the bucket
    pub key: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ScaffoldComponentParams {
    /// Component name, lowercase with hyphens, e.g. "price-feed"
    pub name: String,
    /// Trigger type: evm_contract_event | cosmos_contract_event | block_interval | cron | manual
    pub trigger_type: String,
    /// Optional description of what this component does
    pub description: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SetServiceUriParams {
    /// ServiceManager as a JSON object.
    /// EVM:    `{"evm":{"chain":"evm:31337","address":"0xAbCd..."}}`
    /// Cosmos: `{"cosmos":{"chain":"cosmos:mychain","address":"cosmos1..."}}`
    pub service_manager_json: String,
    /// The URI to set on-chain (e.g. the URL returned by wavs_save_service)
    pub uri: String,
    /// RPC endpoint URL for the chain (e.g. "http://localhost:8545")
    pub rpc_url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DeployServiceManagerParams {
    /// RPC endpoint URL for the chain (e.g. "http://localhost:8545")
    pub rpc_url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DeployPoaServiceManagerParams {
    /// RPC endpoint URL for the chain (e.g. "http://localhost:8545")
    pub rpc_url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RegisterOperatorParams {
    /// ServiceManager as a JSON object.
    /// EVM:    `{"evm":{"chain":"evm:31337","address":"0xAbCd..."}}`
    pub service_manager_json: String,
    /// Weight to assign to the operator (default: 100).
    /// Represents relative stake weight — higher weight = more influence in multi-operator consensus.
    /// For single-operator setups, any positive value works; 100 is conventional.
    pub weight: Option<u64>,
    /// RPC endpoint URL for the chain (e.g. "http://localhost:8545")
    pub rpc_url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BuildComponentParams {
    /// Directory containing the component's `Cargo.toml`
    pub dir: String,
    /// Build in release mode (default: true)
    pub release: Option<bool>,
}

// ── Helpers ────────────────────────────────────────────────────────────────

type McpError = ErrorData;

fn ok(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(text.into())]))
}

fn err(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![Content::text(text.into())]))
}

fn parse_args<T: serde::de::DeserializeOwned>(
    args: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<T, McpError> {
    let value = serde_json::Value::Object(args.unwrap_or_default());
    serde_json::from_value(value).map_err(|e| ErrorData {
        code: ErrorCode::INVALID_PARAMS,
        message: format!("Invalid parameters: {e}").into(),
        data: None,
    })
}

fn no_params() -> Arc<serde_json::Map<String, serde_json::Value>> {
    Arc::new(
        serde_json::json!({"type": "object", "properties": {}})
            .as_object()
            .cloned()
            .unwrap_or_default(),
    )
}

fn tool(name: &'static str, desc: &'static str, schema: Arc<serde_json::Map<String, serde_json::Value>>) -> Tool {
    Tool {
        name: name.into(),
        description: desc.into(),
        input_schema: schema,
    }
}

// ── Server ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct WavsMcpServer {
    client: WavsClient,
    chain_write_credential: Option<String>,
    signing_mnemonic: Option<String>,
}

impl WavsMcpServer {
    pub fn new(
        wavs_url: String,
        token: Option<String>,
        chain_write_credential: Option<String>,
        signing_mnemonic: Option<String>,
    ) -> Self {
        Self {
            client: WavsClient::new(wavs_url, token),
            chain_write_credential,
            signing_mnemonic,
        }
    }

    fn require_chain_write_credential(&self) -> Result<wavs_types::Credential, McpError> {
        self.chain_write_credential
            .as_deref()
            .ok_or_else(|| ErrorData {
                code: ErrorCode::INVALID_PARAMS,
                message: "--chain-write-credential is not configured on this MCP server. \
                    Set WAVS_CHAIN_WRITE_CREDENTIAL env var, add chain_write_credential to wavs.toml [wavs] section, \
                    or restart with --chain-write-credential.".into(),
                data: None,
            })
            .and_then(|s| {
                s.parse::<wavs_types::Credential>().map_err(|e| ErrorData {
                    code: ErrorCode::INVALID_PARAMS,
                    message: format!("invalid chain_write_credential: {e}").into(),
                    data: None,
                })
            })
    }

    fn require_signing_mnemonic(&self) -> Result<wavs_types::Credential, McpError> {
        self.signing_mnemonic
            .as_deref()
            .ok_or_else(|| ErrorData {
                code: ErrorCode::INVALID_PARAMS,
                message: "--signing-mnemonic is not configured on this MCP server. \
                    Set WAVS_SIGNING_MNEMONIC env var, add signing_mnemonic to wavs.toml [wavs] section, \
                    or restart with --signing-mnemonic. \
                    This must be the same mnemonic that the WAVS node uses as its operator signing key.".into(),
                data: None,
            })
            .and_then(|s| {
                s.parse::<wavs_types::Credential>().map_err(|e| ErrorData {
                    code: ErrorCode::INVALID_PARAMS,
                    message: format!("invalid signing_mnemonic: {e}").into(),
                    data: None,
                })
            })
    }

    // ── Tool implementations ───────────────────────────────────────────────

    async fn tool_get_node_info(&self) -> Result<CallToolResult, McpError> {
        match self.client.get_info().await {
            Ok(v) => ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
            Err(e) => err(format!("Failed to get node info: {e:#}")),
        }
    }

    async fn tool_get_health(&self) -> Result<CallToolResult, McpError> {
        match self.client.get_health().await {
            Ok(v) => ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
            Err(e) => err(format!("Failed to get health status: {e:#}")),
        }
    }

    async fn tool_list_services(&self) -> Result<CallToolResult, McpError> {
        match self.client.list_services().await {
            Ok(v) => ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
            Err(e) => err(format!("Failed to list services: {e:#}")),
        }
    }

    async fn tool_get_service(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: GetServiceParams = parse_args(args)?;
        match self.client.get_service(&p.chain, &p.address).await {
            Ok(v) => ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
            Err(e) => err(format!("Failed to get service: {e:#}")),
        }
    }

    async fn tool_deploy_service(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: ServiceManagerParams = parse_args(args)?;
        let manager = match serde_json::from_str(&p.service_manager_json) {
            Ok(m) => m,
            Err(e) => return err(format!("Invalid service_manager_json: {e}")),
        };
        match self.client.deploy_service(manager).await {
            Ok(v) if v.is_null() => ok("Service registered successfully."),
            Ok(v) => ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
            Err(e) => err(format!("Failed to deploy service: {e:#}")),
        }
    }

    async fn tool_pause_service(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: ServiceManagerParams = parse_args(args)?;
        let manager = match serde_json::from_str(&p.service_manager_json) {
            Ok(m) => m,
            Err(e) => return err(format!("Invalid service_manager_json: {e}")),
        };
        match self.client.pause_service(manager).await {
            Ok(()) => ok("Service paused successfully"),
            Err(e) => err(format!("Failed to pause service: {e:#}")),
        }
    }

    async fn tool_resume_service(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: ServiceManagerParams = parse_args(args)?;
        let manager = match serde_json::from_str(&p.service_manager_json) {
            Ok(m) => m,
            Err(e) => return err(format!("Invalid service_manager_json: {e}")),
        };
        match self.client.resume_service(manager).await {
            Ok(()) => ok("Service resumed successfully"),
            Err(e) => err(format!("Failed to resume service: {e:#}")),
        }
    }

    async fn tool_delete_service(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: ServiceManagerParams = parse_args(args)?;
        let manager = match serde_json::from_str(&p.service_manager_json) {
            Ok(m) => m,
            Err(e) => return err(format!("Invalid service_manager_json: {e}")),
        };
        match self.client.delete_service(manager).await {
            Ok(()) => ok("Service deleted successfully"),
            Err(e) => err(format!("Failed to delete service: {e:#}")),
        }
    }

    async fn tool_upload_component(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: UploadComponentParams = parse_args(args)?;
        let bytes = match tokio::fs::read(&p.file_path).await {
            Ok(b) => b,
            Err(e) => return err(format!("Failed to read '{}': {e:#}", p.file_path)),
        };
        match self.client.upload_component(bytes).await {
            Ok(digest) => ok(format!("Component uploaded.\nDigest: {digest}")),
            Err(e) => err(format!("Failed to upload component: {e:#}")),
        }
    }

    async fn tool_save_service(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: SaveServiceParams = parse_args(args)?;
        match self.client.save_service(&p.service_json).await {
            Ok(uri) => ok(format!("Service saved.\nURI: {uri}")),
            Err(e) => err(format!("Failed to save service: {e:#}")),
        }
    }

    async fn tool_simulate_trigger(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        use std::str::FromStr;
        use wavs_types::{ServiceId, WorkflowId};

        let p: SimulateTriggerParams = parse_args(args)?;

        let service_id = ServiceId::from_str(&p.service_id).map_err(|e| ErrorData {
            code: ErrorCode::INVALID_PARAMS,
            message: format!("Invalid service_id '{}': {e}", p.service_id).into(),
            data: None,
        })?;

        let workflow_id = WorkflowId::from_str(&p.workflow_id).map_err(|e| ErrorData {
            code: ErrorCode::INVALID_PARAMS,
            message: format!("Invalid workflow_id '{}': {e}", p.workflow_id).into(),
            data: None,
        })?;

        let trigger = match serde_json::from_str(&p.trigger_json) {
            Ok(t) => t,
            Err(e) => return err(format!("Invalid trigger_json: {e}")),
        };
        let data = match serde_json::from_str(&p.data_json) {
            Ok(d) => d,
            Err(e) => return err(format!("Invalid data_json: {e}")),
        };

        let req = wavs_types::SimulatedTriggerRequest {
            service_id,
            workflow_id,
            trigger,
            data,
            count: p.count.unwrap_or(1).max(1),
            wait_for_completion: false,
        };

        match self.client.simulate_trigger(req).await {
            Ok(()) => ok("Trigger simulated successfully"),
            Err(e) => err(format!("Failed to simulate trigger: {e:#}")),
        }
    }

    async fn tool_deploy_dev_service(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: DeployDevServiceParams = parse_args(args)?;
        match self.client.deploy_dev_service(&p.service_json).await {
            Ok(hash) => ok(format!("Service registered.\nHash: {hash}")),
            Err(e) => err(format!("Failed to deploy dev service: {e:#}")),
        }
    }

    async fn tool_query_kv(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: QueryKvParams = parse_args(args)?;
        match self.client.query_kv(&p.service_id, &p.bucket, &p.key).await {
            Ok(v) => ok(v),
            Err(e) => err(format!("Failed to query KV: {e:#}")),
        }
    }

    async fn tool_set_service_uri(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: SetServiceUriParams = parse_args(args)?;
        let credential = self.require_chain_write_credential()?;
        let manager = match serde_json::from_str(&p.service_manager_json) {
            Ok(m) => m,
            Err(e) => return err(format!("Invalid service_manager_json: {e}")),
        };
        match chain_ops::set_service_uri(&manager, &credential, &p.rpc_url, p.uri).await {
            Ok(()) => ok("Service URI updated on-chain successfully"),
            Err(e) => err(format!("Failed to set service URI: {e:#}")),
        }
    }

    async fn tool_deploy_service_manager(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: DeployServiceManagerParams = parse_args(args)?;
        let credential = self.require_chain_write_credential()?;
        match chain_ops::deploy_service_manager(&credential, &p.rpc_url).await {
            Ok((address, tx_hash)) => ok(format!("SimpleServiceManager deployed.\nAddress: {address}\nTx: {tx_hash}")),
            Err(e) => err(format!("Failed to deploy service manager: {e:#}")),
        }
    }

    async fn tool_deploy_poa_service_manager(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: DeployPoaServiceManagerParams = parse_args(args)?;
        let credential = self.require_chain_write_credential()?;
        match chain_ops::deploy_poa_service_manager(&credential, &p.rpc_url).await {
            Ok(address) => ok(format!("POAStakeRegistry deployed.\nAddress (use as service manager): {address}")),
            Err(e) => err(format!("Failed to deploy POA service manager: {e:#}")),
        }
    }

    async fn tool_register_operator(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: RegisterOperatorParams = parse_args(args)?;
        let owner_cred = self.require_chain_write_credential()?;
        let signing_cred = self.require_signing_mnemonic()?;
        let manager = match serde_json::from_str(&p.service_manager_json) {
            Ok(m) => m,
            Err(e) => return err(format!("Invalid service_manager_json: {e}")),
        };
        let weight = p.weight.unwrap_or(100);
        match chain_ops::register_operator(&manager, &owner_cred, &signing_cred, weight, &p.rpc_url).await {
            Ok((operator, register_tx, signing_key_tx)) => ok(format!(
                "Operator registered.\nOperator: {operator}\nRegister tx: {register_tx}\nSigning key tx: {signing_key_tx}"
            )),
            Err(e) => err(format!("Failed to register operator: {e:#}")),
        }
    }

    async fn tool_get_signing_address(&self) -> Result<CallToolResult, McpError> {
        let credential = self.require_signing_mnemonic()?;
        match chain_ops::get_signing_address(&credential) {
            Ok(addr) => ok(format!("Signing address (HD index 0): {addr}")),
            Err(e) => err(format!("Failed to derive signing address: {e:#}")),
        }
    }

    async fn tool_get_wit_interface(&self) -> Result<CallToolResult, McpError> {
        ok(scaffold::get_wit_interface())
    }

    async fn tool_scaffold_component(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: ScaffoldComponentParams = parse_args(args)?;
        ok(scaffold::scaffold_component(&p.name, &p.trigger_type, p.description.as_deref()))
    }

    async fn tool_build_component(&self, args: Option<serde_json::Map<String, serde_json::Value>>) -> Result<CallToolResult, McpError> {
        let p: BuildComponentParams = parse_args(args)?;
        let release = p.release.unwrap_or(true);

        let mut cmd = tokio::process::Command::new("cargo");
        cmd.arg("component").arg("build");
        if release {
            cmd.arg("--release");
        }
        cmd.current_dir(&p.dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let output = match cmd.output().await {
            Ok(o) => o,
            Err(e) => return err(format!("Failed to run `cargo component build`: {e:#}")),
        };

        let result = format!(
            "Exit code: {}\n\nstdout:\n{}\n\nstderr:\n{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        if output.status.success() { ok(result) } else { err(result) }
    }
}

// ── ServerHandler ──────────────────────────────────────────────────────────

impl ServerHandler for WavsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "wavs-mcp".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            capabilities: ServerCapabilities {
                tools: Some(Default::default()),
                ..Default::default()
            },
            instructions: Some(
                "MCP server for the WAVS (WebAssembly-based Actively Validated Services) platform.\n\
                 \n\
                 Read tools (no auth needed): wavs_get_node_info, wavs_get_health, wavs_list_services, wavs_get_service\n\
                 Write tools (need --token): wavs_deploy_service, wavs_delete_service, wavs_pause_service, wavs_resume_service\n\
                 Dev tools (need dev endpoints): wavs_upload_component, wavs_save_service, wavs_simulate_trigger, wavs_deploy_dev_service, wavs_query_kv\n\
                 Chain-write tools (need WAVS_CHAIN_WRITE_CREDENTIAL on MCP server): wavs_set_service_uri, wavs_deploy_service_manager, wavs_deploy_poa_service_manager\n\
                 Chain-write tools (also need WAVS_SIGNING_MNEMONIC): wavs_register_operator, wavs_get_signing_address\n\
                 Local tools: wavs_get_wit_interface, wavs_scaffold_component, wavs_build_component"
                    .to_string(),
            ),
            ..Default::default()
        }
    }

    async fn list_tools(
        &self,
        _req: PaginatedRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let empty = no_params();

        Ok(ListToolsResult {
            tools: vec![
                // Read tools
                tool("wavs_get_node_info",
                     "Get WAVS node information: service count, chain keys, aggregator config, P2P status",
                     empty.clone()),
                tool("wavs_get_health",
                     "Get health status of all configured chain RPC endpoints",
                     empty.clone()),
                tool("wavs_list_services",
                     "List all registered services with their workflows, triggers, and components",
                     empty.clone()),
                Tool {
                    name: "wavs_get_service".into(),
                    description: "Get full configuration for a specific service by chain and address".into(),
                    input_schema: schema_for_type::<GetServiceParams>().into(),
                },
                // Write tools
                Tool {
                    name: "wavs_deploy_service".into(),
                    description: "Register a service by its ServiceManager. Pass service_manager_json as: \
                        {\"evm\":{\"chain\":\"evm:31337\",\"address\":\"0x...\"}} or \
                        {\"cosmos\":{\"chain\":\"cosmos:mychain\",\"address\":\"cosmos1...\"}}. Requires --token.".into(),
                    input_schema: schema_for_type::<ServiceManagerParams>().into(),
                },
                Tool {
                    name: "wavs_delete_service".into(),
                    description: "Delete a registered service. Requires --token.".into(),
                    input_schema: schema_for_type::<ServiceManagerParams>().into(),
                },
                Tool {
                    name: "wavs_pause_service".into(),
                    description: "Pause a registered service. Requires --token.".into(),
                    input_schema: schema_for_type::<ServiceManagerParams>().into(),
                },
                Tool {
                    name: "wavs_resume_service".into(),
                    description: "Resume a paused service. Requires --token.".into(),
                    input_schema: schema_for_type::<ServiceManagerParams>().into(),
                },
                // Chain-write tools (need WAVS_CHAIN_WRITE_CREDENTIAL on MCP server)
                Tool {
                    name: "wavs_set_service_uri".into(),
                    description: "Call setServiceURI on the ServiceManager contract to update the \
                        on-chain service URI. Requires --chain-write-credential (WAVS_CHAIN_WRITE_CREDENTIAL) \
                        to be configured on this MCP server. Provide the chain RPC URL as rpc_url. \
                        EVM only currently.".into(),
                    input_schema: schema_for_type::<SetServiceUriParams>().into(),
                },
                Tool {
                    name: "wavs_deploy_service_manager".into(),
                    description: "Deploy a new SimpleServiceManager PoA contract on-chain and return its address. \
                        Requires --chain-write-credential (WAVS_CHAIN_WRITE_CREDENTIAL) on this MCP server. \
                        Provide the chain RPC URL as rpc_url. EVM only currently.".into(),
                    input_schema: schema_for_type::<DeployServiceManagerParams>().into(),
                },
                Tool {
                    name: "wavs_deploy_poa_service_manager".into(),
                    description: "Deploy a new POAStakeRegistry (full PoA middleware with proxy) on-chain via Docker. \
                        Returns the proxy address to use as service manager. \
                        Requires --chain-write-credential on this MCP server. \
                        Docker image ghcr.io/lay3rlabs/poa-middleware:1.0.1 must be available. \
                        Provide the chain RPC URL as rpc_url. EVM only currently. \
                        After deploying, call wavs_register_operator with the returned address to register the \
                        WAVS node's signing key as an operator before the service can process triggers.".into(),
                    input_schema: schema_for_type::<DeployPoaServiceManagerParams>().into(),
                },
                Tool {
                    name: "wavs_register_operator".into(),
                    description: "Step 2 of PoA setup (call after wavs_deploy_poa_service_manager). \
                        Registers the WAVS node's signing key as an operator on a POAStakeRegistry contract \
                        and sets the signing key mapping. \
                        Calls registerOperator (using WAVS_CHAIN_WRITE_CREDENTIAL as owner) and \
                        updateOperatorSigningKey (using WAVS_SIGNING_MNEMONIC as operator). \
                        weight is a relative stake weight (default: 100; any positive value works for single-operator setups). \
                        Requires --chain-write-credential and --signing-mnemonic on this MCP server. \
                        Provide the chain RPC URL as rpc_url. EVM only currently.".into(),
                    input_schema: schema_for_type::<RegisterOperatorParams>().into(),
                },
                Tool {
                    name: "wavs_get_signing_address".into(),
                    description: "Get the EVM address of the WAVS node's signing key, derived from the configured \
                        signing mnemonic (HD index 0). \
                        Useful for verifying operator registration. \
                        Requires --signing-mnemonic (WAVS_SIGNING_MNEMONIC) to be configured on this MCP server.".into(),
                    input_schema: no_params(),
                },
                // Dev tools
                Tool {
                    name: "wavs_upload_component".into(),
                    description: "Upload a compiled .wasm binary to the WAVS node. Returns the component digest. \
                        Requires dev endpoints enabled in wavs.toml.".into(),
                    input_schema: schema_for_type::<UploadComponentParams>().into(),
                },
                Tool {
                    name: "wavs_save_service".into(),
                    description: "Save a service definition to the WAVS node's local store without registering it. \
                        Returns the URI (e.g. http://localhost:8000/dev/services/<hash>) that can be set as the \
                        on-chain serviceURI so the service can later be registered via wavs_deploy_service. \
                        Requires dev endpoints enabled in wavs.toml.".into(),
                    input_schema: schema_for_type::<SaveServiceParams>().into(),
                },
                Tool {
                    name: "wavs_simulate_trigger".into(),
                    description: "Simulate a trigger against a deployed service. \
                        Requires dev endpoints enabled in wavs.toml.".into(),
                    input_schema: schema_for_type::<SimulateTriggerParams>().into(),
                },
                Tool {
                    name: "wavs_deploy_dev_service".into(),
                    description: "Register a service directly without an on-chain contract (dev/testing only). \
                        Pass the full Service JSON. Handles the two-step save+register flow internally. \
                        Requires dev endpoints enabled in wavs.toml and --token.".into(),
                    input_schema: schema_for_type::<DeployDevServiceParams>().into(),
                },
                Tool {
                    name: "wavs_query_kv".into(),
                    description: "Read a value from a service's KV store. \
                        Useful for inspecting state written by kv-store components. \
                        Requires dev endpoints enabled in wavs.toml.".into(),
                    input_schema: schema_for_type::<QueryKvParams>().into(),
                },
                // Local tools
                tool("wavs_get_wit_interface",
                     "Return the full WIT interface definitions for WAVS WASM components \
                      (HTTP, KV, sockets, TLS, host functions, etc.)",
                     empty.clone()),
                Tool {
                    name: "wavs_scaffold_component".into(),
                    description: "Generate a ready-to-build WAVS WASM component scaffold (Cargo.toml + lib.rs). \
                        Trigger types: evm_contract_event | cosmos_contract_event | block_interval | cron | manual".into(),
                    input_schema: schema_for_type::<ScaffoldComponentParams>().into(),
                },
                Tool {
                    name: "wavs_build_component".into(),
                    description: "Build a WAVS WASM component using `cargo component build`. \
                        Returns full build output.".into(),
                    input_schema: schema_for_type::<BuildComponentParams>().into(),
                },
            ],
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let args = req.arguments;
        match req.name.as_ref() {
            "wavs_get_node_info"      => self.tool_get_node_info().await,
            "wavs_get_health"         => self.tool_get_health().await,
            "wavs_list_services"      => self.tool_list_services().await,
            "wavs_get_service"        => self.tool_get_service(args).await,
            "wavs_deploy_service"     => self.tool_deploy_service(args).await,
            "wavs_delete_service"     => self.tool_delete_service(args).await,
            "wavs_pause_service"      => self.tool_pause_service(args).await,
            "wavs_resume_service"     => self.tool_resume_service(args).await,
            "wavs_set_service_uri"              => self.tool_set_service_uri(args).await,
            "wavs_deploy_service_manager"       => self.tool_deploy_service_manager(args).await,
            "wavs_deploy_poa_service_manager"   => self.tool_deploy_poa_service_manager(args).await,
            "wavs_register_operator"            => self.tool_register_operator(args).await,
            "wavs_get_signing_address"          => self.tool_get_signing_address().await,
            "wavs_upload_component"        => self.tool_upload_component(args).await,
            "wavs_save_service"          => self.tool_save_service(args).await,
            "wavs_simulate_trigger"      => self.tool_simulate_trigger(args).await,
            "wavs_deploy_dev_service"    => self.tool_deploy_dev_service(args).await,
            "wavs_query_kv"              => self.tool_query_kv(args).await,
            "wavs_get_wit_interface"     => self.tool_get_wit_interface().await,
            "wavs_scaffold_component" => self.tool_scaffold_component(args).await,
            "wavs_build_component"    => self.tool_build_component(args).await,
            name => Err(ErrorData {
                code: ErrorCode::METHOD_NOT_FOUND,
                message: format!("Unknown tool: {name}").into(),
                data: None,
            }),
        }
    }
}
