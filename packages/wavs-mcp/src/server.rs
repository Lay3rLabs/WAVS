use std::sync::Arc;
use std::time::Duration;

use rmcp::{
    handler::server::tool::schema_for_type,
    model::*,
    schemars,
    service::{Peer, RequestContext, RoleServer},
    ServerHandler,
};
use serde::Deserialize;

use crate::chain_ops;
use crate::client::WavsClient;
use crate::exec;

/// Serde helper: deserialize a number that may arrive as a JSON string (LLMs often quote numbers).
mod string_or_number {
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize_option_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrNum {
            Num(usize),
            Str(String),
        }
        let opt: Option<StringOrNum> = Option::deserialize(deserializer)?;
        match opt {
            None => Ok(None),
            Some(StringOrNum::Num(n)) => Ok(Some(n)),
            Some(StringOrNum::Str(s)) => s
                .parse::<usize>()
                .map(Some)
                .map_err(serde::de::Error::custom),
        }
    }

    pub fn deserialize_option_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrNum {
            Num(u64),
            Str(String),
        }
        let opt: Option<StringOrNum> = Option::deserialize(deserializer)?;
        match opt {
            None => Ok(None),
            Some(StringOrNum::Num(n)) => Ok(Some(n)),
            Some(StringOrNum::Str(s)) => {
                s.parse::<u64>().map(Some).map_err(serde::de::Error::custom)
            }
        }
    }

    pub fn deserialize_option_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrNum {
            Num(u32),
            Str(String),
        }
        let opt: Option<StringOrNum> = Option::deserialize(deserializer)?;
        match opt {
            None => Ok(None),
            Some(StringOrNum::Num(n)) => Ok(Some(n)),
            Some(StringOrNum::Str(s)) => {
                s.parse::<u32>().map(Some).map_err(serde::de::Error::custom)
            }
        }
    }
}
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
    /// Service ID — 64-char hex string derived from the ServiceManager.
    /// This is returned by wavs_deploy_dev_service as `service_id`.
    /// NOT the deploy_hash — the service_id is a different value.
    pub service_id: String,
    /// Workflow ID — lowercase alphanumeric, 3–36 chars (e.g. "default")
    pub workflow_id: String,
    /// Trigger definition as JSON, e.g. `{"cron":{"schedule":"* * * * * * *","start_time":null,"end_time":null}}`
    /// Note: cron schedule uses 7-field format: `sec min hour dom month dow year`
    pub trigger_json: String,
    /// TriggerData as JSON, e.g. `{"Cron":{"trigger_time":0}}`
    pub data_json: String,
    /// How many times to fire the trigger (default: 1)
    #[serde(
        default,
        deserialize_with = "string_or_number::deserialize_option_usize"
    )]
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
pub struct QueryLogsParams {
    /// Return only entries with id >= since_id. Pass the `next_id` from the previous response
    /// to page forward. Defaults to 0 (return from the oldest buffered entry).
    #[serde(default, deserialize_with = "string_or_number::deserialize_option_u64")]
    pub since_id: Option<u64>,
    /// Maximum number of entries to return (default: 100, max: 1000).
    #[serde(
        default,
        deserialize_with = "string_or_number::deserialize_option_usize"
    )]
    pub limit: Option<usize>,
    /// Minimum log level filter: trace | debug | info | warn | error.
    /// Returns entries at this level and above (e.g. "info" includes warn + error).
    pub level: Option<String>,
    /// Filter by target prefix, e.g. "wavs" or "wavs::subsystems::engine".
    /// Component logs appear under "wavs::subsystems::engine::wasm_engine".
    pub target: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct QueryComponentLogsParams {
    /// Return only entries with id >= since_id. Pass the `next_id` from the previous response
    /// to page forward. Defaults to 0 (return from the oldest buffered entry).
    #[serde(default, deserialize_with = "string_or_number::deserialize_option_u64")]
    pub since_id: Option<u64>,
    /// Maximum number of entries to return (default: 100, max: 1000).
    #[serde(
        default,
        deserialize_with = "string_or_number::deserialize_option_usize"
    )]
    pub limit: Option<usize>,
    /// Minimum log level filter: trace | debug | info | warn | error.
    pub level: Option<String>,
    /// Filter to logs from a specific service (64-char hex service ID).
    pub service_id: Option<String>,
    /// Filter to logs from a specific workflow ID, e.g. "default".
    pub workflow_id: Option<String>,
    /// Filter to logs from a specific component digest (sha256 hex string).
    pub digest: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ScaffoldComponentParams {
    /// Component name, lowercase with hyphens, e.g. "price-feed"
    pub name: String,
    /// Trigger type: evm_contract_event | cosmos_contract_event | block_interval | cron | manual
    pub trigger_type: String,
    /// Directory to create the project in. The component directory `{dir}/{name}/` will be created.
    /// If omitted, returns the file contents as text instead of writing to disk.
    /// Example: "/tmp" creates "/tmp/price-feed/"
    pub dir: Option<String>,
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
    #[serde(default, deserialize_with = "string_or_number::deserialize_option_u64")]
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

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ValidateComponentParams {
    /// Path to the compiled .wasm component file
    pub wasm_path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GetSigningAddressParams {
    /// HD derivation index to use (default: 0). Use the hd_index reported by
    /// wavs_get_service_signer to check a service-specific signing key.
    #[serde(default, deserialize_with = "string_or_number::deserialize_option_u32")]
    pub hd_index: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DeployAndRegisterParams {
    /// ServiceManager as a JSON object.
    /// EVM: `{"evm":{"chain":"evm:31337","address":"0xAbCd..."}}`
    pub service_manager_json: String,
    /// Weight to assign to the operator (default: 100).
    #[serde(default, deserialize_with = "string_or_number::deserialize_option_u64")]
    pub weight: Option<u64>,
    /// RPC endpoint URL for the chain (e.g. "http://localhost:8545")
    pub rpc_url: String,
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
    let mut map = args.unwrap_or_default();
    // MCP clients (especially Claude) often send bools and numbers as strings.
    // Coerce string values that look like bools/numbers to their native JSON types.
    coerce_string_values(&mut map);
    let value = serde_json::Value::Object(map);
    serde_json::from_value(value).map_err(|e| ErrorData {
        code: ErrorCode::INVALID_PARAMS,
        message: format!("Invalid parameters: {e}").into(),
        data: None,
    })
}

/// Coerce string values that look like bools or numbers to native JSON types.
/// Handles: "true"/"false" → bool, "123" → number, "1.5" → number.
/// Only applies to top-level string values (not nested objects/arrays).
fn coerce_string_values(map: &mut serde_json::Map<String, serde_json::Value>) {
    for value in map.values_mut() {
        if let serde_json::Value::String(s) = value {
            match s.as_str() {
                "true" => *value = serde_json::Value::Bool(true),
                "false" => *value = serde_json::Value::Bool(false),
                other => {
                    if let Ok(n) = other.parse::<u64>() {
                        *value = serde_json::Value::Number(n.into());
                    } else if let Ok(n) = other.parse::<f64>() {
                        if let Some(n) = serde_json::Number::from_f64(n) {
                            *value = serde_json::Value::Number(n);
                        }
                    }
                }
            }
        }
    }
}

/// Detect placeholder/example addresses that agents copy verbatim from schema examples.
/// Matches patterns like 0x1234567890..., 0xAbCdEf..., 0xServiceManagerAddress, etc.
fn is_placeholder_address(addr: &str) -> bool {
    let lower = addr.to_lowercase();
    // Non-hex characters in the address part → clearly a placeholder like "0xServiceManagerAddress"
    if let Some(hex_part) = lower.strip_prefix("0x") {
        if hex_part.chars().any(|c| !c.is_ascii_hexdigit()) {
            return true;
        }
    }
    // Common sequential/repeating patterns agents generate
    let patterns = [
        "0x1234567890",
        "0xabcdef1234",
        "0x0000000000",
        "0xaaaaaaaaaa",
        "0x1111111111",
    ];
    for p in patterns {
        if lower.starts_with(p) {
            return true;
        }
    }
    false
}

/// Generate a unique hex string of the given length for use as a dev manager address.
/// Uses timestamp + process ID + counter for uniqueness (no `rand` crate needed).
fn random_hex(len: usize) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let pid = std::process::id() as u64;
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    // Hash the values together to produce enough hex chars
    let mut s = format!("{:016x}{:08x}{:016x}", nanos, pid, count);
    s.truncate(len);
    s
}

fn no_params() -> Arc<serde_json::Map<String, serde_json::Value>> {
    Arc::new(
        serde_json::json!({"type": "object", "properties": {}})
            .as_object()
            .cloned()
            .unwrap_or_default(),
    )
}

fn tool(
    name: &'static str,
    desc: &'static str,
    schema: Arc<serde_json::Map<String, serde_json::Value>>,
) -> Tool {
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
    mcp_chain_credential: Option<String>,
    signing_mnemonic: Option<String>,
    exec_enabled: bool,
    service_cache: Arc<exec::ServiceCache>,
    peer: Arc<tokio::sync::RwLock<Option<Peer<RoleServer>>>>,
    pending_confirmations: Arc<exec::PendingConfirmations>,
}

impl WavsMcpServer {
    pub fn new(
        wavs_url: String,
        token: Option<String>,
        mcp_chain_credential: Option<String>,
        signing_mnemonic: Option<String>,
        exec_enabled: bool,
    ) -> Self {
        Self {
            client: WavsClient::new(wavs_url, token),
            mcp_chain_credential,
            signing_mnemonic,
            exec_enabled,
            service_cache: Arc::new(exec::ServiceCache::new(Duration::from_secs(5))),
            peer: Arc::new(tokio::sync::RwLock::new(None)),
            pending_confirmations: Arc::new(exec::PendingConfirmations::new()),
        }
    }

    fn require_mcp_chain_credential(&self) -> Result<wavs_types::Credential, McpError> {
        self.mcp_chain_credential
            .as_deref()
            .ok_or_else(|| ErrorData {
                code: ErrorCode::INVALID_PARAMS,
                message: "--mcp-chain-credential is not configured on this MCP server. \
                    Set WAVS_MCP_CHAIN_CREDENTIAL env var in the MCP client config, \
                    or restart with --mcp-chain-credential."
                    .into(),
                data: None,
            })
            .and_then(|s| {
                s.parse::<wavs_types::Credential>().map_err(|e| ErrorData {
                    code: ErrorCode::INVALID_PARAMS,
                    message: format!("invalid mcp_chain_credential: {e}").into(),
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

    // ── Service cache helpers ──────────────────────────────────────────────

    async fn get_services_cached(&self) -> Result<serde_json::Value, McpError> {
        if let Some(cached) = self.service_cache.get().await {
            return Ok(cached);
        }
        let services = self.client.list_services().await.map_err(|e| ErrorData {
            code: ErrorCode::INTERNAL_ERROR,
            message: format!("Failed to fetch services: {e:#}").into(),
            data: None,
        })?;
        self.service_cache.set(services.clone()).await;
        Ok(services)
    }

    /// Invalidate the service cache and notify the MCP client that the tool
    /// list has changed. Called after deploy/delete operations.
    async fn notify_tools_changed(&self) {
        self.service_cache.invalidate().await;
        if let Some(peer) = self.peer.try_read().ok().and_then(|g| g.clone()) {
            if let Err(e) = peer.notify_tool_list_changed().await {
                tracing::warn!("Failed to send tools/list_changed notification: {e}");
            }
        }
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

    async fn tool_get_service(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: GetServiceParams = parse_args(args)?;
        match self.client.get_service(&p.chain, &p.address).await {
            Ok(v) => ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
            Err(e) => err(format!("Failed to get service: {e:#}")),
        }
    }

    async fn tool_deploy_service(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: ServiceManagerParams = parse_args(args)?;
        let manager: wavs_types::ServiceManager =
            match serde_json::from_str(&p.service_manager_json) {
                Ok(m) => m,
                Err(e) => return err(format!("Invalid service_manager_json: {e}")),
            };
        let nav_directive = match &manager {
            wavs_types::ServiceManager::Evm { chain, address } => {
                format!(
                    "\n\n⚡ ACTION REQUIRED: If you have the ui_navigate tool, call it NOW with path=\"/services/{chain}/{address}\" \
                     to show the user their new service. Do this BEFORE any other step."
                )
            }
            wavs_types::ServiceManager::Cosmos { chain, address } => {
                format!(
                    "\n\n⚡ ACTION REQUIRED: If you have the ui_navigate tool, call it NOW with path=\"/services/{chain}/{address}\" \
                     to show the user their new service. Do this BEFORE any other step."
                )
            }
        };

        match self.client.deploy_service(manager.clone()).await {
            Ok(v) if v.is_null() => {
                let signer_info = match self.client.get_service_signer(manager).await {
                    Ok(wavs_types::SignerResponse::Secp256k1 {
                        hd_index,
                        evm_address,
                    }) => {
                        format!("\nSigning key: HD index {hd_index} ({evm_address})\nCall wavs_register_operator next if using PoA.")
                    }
                    Err(_) => String::new(),
                };
                self.notify_tools_changed().await;
                ok(format!(
                    "Service registered successfully.{nav_directive}{signer_info}"
                ))
            }
            Ok(v) => {
                self.notify_tools_changed().await;
                ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()))
            }
            Err(e) => err(format!("Failed to deploy service: {e:#}")),
        }
    }

    async fn tool_delete_service(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: ServiceManagerParams = parse_args(args)?;
        let manager = match serde_json::from_str(&p.service_manager_json) {
            Ok(m) => m,
            Err(e) => return err(format!("Invalid service_manager_json: {e}")),
        };
        match self.client.delete_service(manager).await {
            Ok(()) => {
                self.notify_tools_changed().await;
                ok("Service deleted successfully")
            }
            Err(e) => err(format!("Failed to delete service: {e:#}")),
        }
    }

    async fn tool_upload_component(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
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

    async fn tool_save_service(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: SaveServiceParams = parse_args(args)?;
        match self.client.save_service(&p.service_json).await {
            Ok(uri) => ok(format!("Service saved.\nURI: {uri}")),
            Err(e) => err(format!("Failed to save service: {e:#}")),
        }
    }

    async fn tool_simulate_trigger(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
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

    async fn tool_deploy_dev_service(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: DeployDevServiceParams = parse_args(args)?;

        // Parse the service JSON
        let mut service_value: serde_json::Value =
            serde_json::from_str(&p.service_json).map_err(|e| ErrorData {
                code: ErrorCode::INVALID_PARAMS,
                message: format!("Invalid service JSON: {e}").into(),
                data: None,
            })?;

        // For dev services: replace placeholder manager addresses with unique random ones.
        // This prevents "already registered" errors when agents copy example addresses verbatim.
        let manager_replaced = if let Some(addr) = service_value
            .pointer("/manager/evm/address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            if is_placeholder_address(&addr) {
                let random_addr = format!("0x{}", random_hex(40));
                service_value["manager"]["evm"]["address"] = serde_json::Value::String(random_addr);
                true
            } else {
                false
            }
        } else {
            false
        };

        let service_json = serde_json::to_string(&service_value).unwrap();

        let manager: Option<wavs_types::ServiceManager> = service_value
            .get("manager")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        // Extract workflow IDs for the summary
        let workflow_ids: Vec<String> = service_value
            .get("workflows")
            .and_then(|v| v.as_object())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();

        match self.client.deploy_dev_service(&service_json).await {
            Ok(hash) => {
                // Compute the service_id from the ServiceManager
                let service_id_info = if let Some(ref mgr) = manager {
                    let sid = wavs_types::ServiceId::from(mgr);
                    format!(
                        "\nservice_id: {sid}  ← use this for wavs_simulate_trigger and wavs_query_component_logs"
                    )
                } else {
                    String::new()
                };

                let manager_info = if manager_replaced {
                    let addr = service_value["manager"]["evm"]["address"]
                        .as_str()
                        .unwrap_or("unknown");
                    format!(
                        "\nmanager_address: {addr}  (placeholder was replaced with unique address)"
                    )
                } else {
                    String::new()
                };

                let workflow_info = if !workflow_ids.is_empty() {
                    format!(
                        "\nworkflow_id(s): {}",
                        workflow_ids
                            .iter()
                            .map(|w| format!("\"{}\"", w))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else {
                    String::new()
                };

                let signer_info = if let Some(mgr) = manager {
                    match self.client.get_service_signer(mgr).await {
                        Ok(wavs_types::SignerResponse::Secp256k1 {
                            hd_index,
                            evm_address,
                        }) => format!("\nsigning_key: HD index {hd_index} ({evm_address})"),
                        Err(_) => String::new(),
                    }
                } else {
                    String::new()
                };

                // Build the ui_navigate action directive
                let nav_action = service_value
                    .pointer("/manager/evm")
                    .and_then(|evm| {
                        let chain = evm.get("chain")?.as_str()?;
                        let addr = evm.get("address")?.as_str()?;
                        Some(format!("/services/{chain}/{addr}"))
                    })
                    .or_else(|| {
                        service_value.pointer("/manager/cosmos").and_then(|cosmos| {
                            let chain = cosmos.get("chain")?.as_str()?;
                            let addr = cosmos.get("address")?.as_str()?;
                            Some(format!("/services/{chain}/{addr}"))
                        })
                    });

                let nav_directive = if let Some(path) = nav_action {
                    format!(
                        "\n\n⚡ ACTION REQUIRED: If you have the ui_navigate tool, call it NOW with path=\"{path}\" \
                         to show the user their new service. Do this BEFORE simulate_trigger or any other step."
                    )
                } else {
                    String::new()
                };

                self.notify_tools_changed().await;
                ok(format!(
                    "✅ Service deployed successfully.\
                     {nav_directive}\n\n\
                     deploy_hash: {hash}\
                     {service_id_info}\
                     {manager_info}\
                     {workflow_info}\
                     {signer_info}"
                ))
            }
            Err(e) => err(format!("Failed to deploy dev service: {e:#}")),
        }
    }

    async fn tool_query_kv(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: QueryKvParams = parse_args(args)?;
        match self.client.query_kv(&p.service_id, &p.bucket, &p.key).await {
            Ok(v) => ok(v),
            Err(e) => err(format!("Failed to query KV: {e:#}")),
        }
    }

    async fn tool_query_logs(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: QueryLogsParams = parse_args(args)?;
        match self
            .client
            .query_logs(
                p.since_id.unwrap_or(0),
                p.limit,
                p.level.as_deref(),
                p.target.as_deref(),
            )
            .await
        {
            Ok(v) => ok(serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())),
            Err(e) => err(format!("Failed to query logs: {e:#}")),
        }
    }

    async fn tool_query_component_logs(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: QueryComponentLogsParams = parse_args(args)?;
        match self
            .client
            .query_logs(
                p.since_id.unwrap_or(0),
                p.limit,
                p.level.as_deref(),
                Some("wavs::subsystems::engine::wasm_engine"),
            )
            .await
        {
            Ok(v) => {
                let entries = v["entries"].as_array().cloned().unwrap_or_default();
                // Match a structured field by key at a boundary (start of the
                // fields string or immediately after the ", " separator) to
                // avoid false positives when the message value itself contains
                // text that looks like a key=value pair.
                let has_field = |fields: &str, key: &str, val: &str| -> bool {
                    let unquoted = format!("{key}={val}");
                    let quoted = format!("{key}=\"{val}\"");
                    fields.starts_with(&unquoted)
                        || fields.starts_with(&quoted)
                        || fields.contains(&format!(", {unquoted}"))
                        || fields.contains(&format!(", {quoted}"))
                };
                let filtered: Vec<_> = entries
                    .into_iter()
                    .filter(|e| {
                        let fields = e["fields"].as_str().unwrap_or("");
                        if let Some(sid) = &p.service_id {
                            if !has_field(fields, "service_id", sid) {
                                return false;
                            }
                        }
                        if let Some(wid) = &p.workflow_id {
                            if !has_field(fields, "workflow_id", wid) {
                                return false;
                            }
                        }
                        if let Some(d) = &p.digest {
                            if !has_field(fields, "digest", d) {
                                return false;
                            }
                        }
                        true
                    })
                    .collect();
                let result = serde_json::json!({ "entries": filtered, "next_id": v["next_id"] });
                ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
            }
            Err(e) => err(format!("Failed to query component logs: {e:#}")),
        }
    }

    async fn tool_set_service_uri(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: SetServiceUriParams = parse_args(args)?;
        let credential = self.require_mcp_chain_credential()?;
        let manager = match serde_json::from_str(&p.service_manager_json) {
            Ok(m) => m,
            Err(e) => return err(format!("Invalid service_manager_json: {e}")),
        };
        match chain_ops::set_service_uri(&manager, &credential, &p.rpc_url, p.uri).await {
            Ok(()) => ok("Service URI updated on-chain successfully"),
            Err(e) => err(format!("Failed to set service URI: {e:#}")),
        }
    }

    async fn tool_deploy_service_manager(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: DeployServiceManagerParams = parse_args(args)?;
        let credential = self.require_mcp_chain_credential()?;
        match chain_ops::deploy_service_manager(&credential, &p.rpc_url).await {
            Ok((address, tx_hash)) => ok(format!(
                "SimpleServiceManager deployed.\nAddress: {address}\nTx: {tx_hash}"
            )),
            Err(e) => err(format!("Failed to deploy service manager: {e:#}")),
        }
    }

    async fn tool_deploy_poa_service_manager(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: DeployPoaServiceManagerParams = parse_args(args)?;
        let credential = self.require_mcp_chain_credential()?;
        match chain_ops::deploy_poa_service_manager(&credential, &p.rpc_url).await {
            Ok(address) => ok(format!(
                "POAStakeRegistry deployed.\nAddress (use as service manager): {address}"
            )),
            Err(e) => err(format!("Failed to deploy POA service manager: {e:#}")),
        }
    }

    async fn tool_register_operator(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: RegisterOperatorParams = parse_args(args)?;
        let owner_cred = self.require_mcp_chain_credential()?;
        let signing_cred = self.require_signing_mnemonic()?;
        let manager: wavs_types::ServiceManager =
            match serde_json::from_str(&p.service_manager_json) {
                Ok(m) => m,
                Err(e) => return err(format!("Invalid service_manager_json: {e}")),
            };
        let weight = p.weight.unwrap_or(100);

        // Query the WAVS node for the HD index assigned to this service. The node assigns a unique
        // HD-derived signing key per service (starting at index 1), so we must register the correct
        // address on-chain. This requires the service to be deployed to the node first.
        let signing_key_hd_index = match self.client.get_service_signer(manager.clone()).await {
            Ok(wavs_types::SignerResponse::Secp256k1 {
                hd_index,
                evm_address,
            }) => {
                tracing::info!("Service signing key: HD index {hd_index} → {evm_address}");
                hd_index
            }
            Err(e) => {
                return err(format!(
                    "Failed to query service signing key from WAVS node: {e:#}\n\n\
                     wavs_register_operator must be called AFTER wavs_deploy_service (or \
                     wavs_deploy_dev_service) so the node has assigned a signing key to the \
                     service. Deploy the service first, then call wavs_register_operator."
                ));
            }
        };

        match chain_ops::register_operator(&manager, &owner_cred, &signing_cred, weight, signing_key_hd_index, &p.rpc_url).await {
            Ok((signing_key, register_tx, signing_key_tx)) => ok(format!(
                "Operator registered.\nSigning key (HD index {signing_key_hd_index}): {signing_key}\nRegister tx: {register_tx}\nSigning key tx: {signing_key_tx}"
            )),
            Err(e) => err(format!("Failed to register operator: {e:#}")),
        }
    }

    async fn tool_get_signing_address(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: GetSigningAddressParams = parse_args(args)?;
        let credential = self.require_signing_mnemonic()?;
        let hd_index = p.hd_index.unwrap_or(0);
        match chain_ops::get_signing_address(&credential, Some(hd_index)) {
            Ok(addr) => ok(format!("Signing address (HD index {hd_index}): {addr}")),
            Err(e) => err(format!("Failed to derive signing address: {e:#}")),
        }
    }

    async fn tool_get_service_signer(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: ServiceManagerParams = parse_args(args)?;
        let manager = match serde_json::from_str(&p.service_manager_json) {
            Ok(m) => m,
            Err(e) => return err(format!("Invalid service_manager_json: {e}")),
        };
        match self.client.get_service_signer(manager).await {
            Ok(wavs_types::SignerResponse::Secp256k1 {
                hd_index,
                evm_address,
            }) => ok(format!(
                "Service signing key:\n  HD index:    {hd_index}\n  EVM address: {evm_address}"
            )),
            Err(e) => err(format!("Failed to get service signer: {e:#}")),
        }
    }

    async fn tool_deploy_and_register(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: DeployAndRegisterParams = parse_args(args)?;
        let owner_cred = self.require_mcp_chain_credential()?;
        let signing_cred = self.require_signing_mnemonic()?;
        let manager: wavs_types::ServiceManager =
            match serde_json::from_str(&p.service_manager_json) {
                Ok(m) => m,
                Err(e) => return err(format!("Invalid service_manager_json: {e}")),
            };

        // Step 1: register the service with the WAVS node so it gets an HD index assigned.
        match self.client.deploy_service(manager.clone()).await {
            Ok(v) if v.is_null() => {}
            Ok(v) => tracing::info!("deploy_service response: {v}"),
            Err(e) => return err(format!("wavs_deploy_service failed: {e:#}")),
        }

        // Step 2: query the node for the service-specific signing key.
        let hd_index = match self.client.get_service_signer(manager.clone()).await {
            Ok(wavs_types::SignerResponse::Secp256k1 { hd_index, .. }) => hd_index,
            Err(e) => {
                return err(format!(
                    "Service deployed but could not query signing key: {e:#}\n\
                 Run wavs_register_operator separately once the node is ready."
                ))
            }
        };

        // Step 3: register the operator on-chain with the correct signing key.
        let weight = p.weight.unwrap_or(100);
        match chain_ops::register_operator(
            &manager,
            &owner_cred,
            &signing_cred,
            weight,
            hd_index,
            &p.rpc_url,
        )
        .await
        {
            Ok((signing_key, register_tx, signing_key_tx)) => ok(format!(
                "Service deployed and operator registered.\n\
                 Signing key (HD index {hd_index}): {signing_key}\n\
                 Register tx:    {register_tx}\n\
                 Signing key tx: {signing_key_tx}"
            )),
            Err(e) => err(format!(
                "Service deployed (HD index {hd_index}) but operator registration failed: {e:#}\n\
                 Run wavs_register_operator separately to retry the on-chain step."
            )),
        }
    }

    async fn tool_get_wit_interface(&self) -> Result<CallToolResult, McpError> {
        ok(scaffold::get_wit_interface())
    }

    async fn tool_scaffold_component(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: ScaffoldComponentParams = parse_args(args)?;
        if let Some(dir) = &p.dir {
            // Write files to disk
            match scaffold::scaffold_component_to_disk(
                &p.name,
                &p.trigger_type,
                dir,
                p.description.as_deref(),
            ) {
                Ok(summary) => ok(summary),
                Err(e) => err(format!("Failed to scaffold component: {e}")),
            }
        } else {
            // Return file contents as text
            ok(scaffold::scaffold_component_text(
                &p.name,
                &p.trigger_type,
                p.description.as_deref(),
            ))
        }
    }

    async fn tool_build_component(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: BuildComponentParams = parse_args(args)?;
        let release = p.release.unwrap_or(true);

        // Detect standalone vs workspace project.
        // Standalone projects have a local `wit/` directory and no `[package.metadata.component]`
        // with `package = "component:..."` that cargo-component uses.
        // For standalone, use `cargo build --target wasm32-wasip2`.
        // For workspace, use `cargo component build`.
        let dir_path = std::path::Path::new(&p.dir);
        let has_local_wit = dir_path.join("wit").is_dir();
        let cargo_toml_path = dir_path.join("Cargo.toml");
        let has_component_metadata = std::fs::read_to_string(&cargo_toml_path)
            .map(|s| s.contains("[package.metadata.component]"))
            .unwrap_or(false);

        // Use standalone build (wasm32-wasip2) when:
        // - Project has local wit/ directory AND no component metadata, OR
        // - Project has local wit/ directory AND is not in a cargo workspace
        let use_standalone = has_local_wit && !has_component_metadata;

        let mut cmd = tokio::process::Command::new("cargo");
        if use_standalone {
            cmd.arg("build").arg("--target").arg("wasm32-wasip2");
        } else {
            cmd.arg("component").arg("build");
        }
        if release {
            cmd.arg("--release");
        }
        cmd.current_dir(&p.dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let build_cmd_str = if use_standalone {
            "cargo build --target wasm32-wasip2"
        } else {
            "cargo component build"
        };

        let output = match cmd.output().await {
            Ok(o) => o,
            Err(e) => return err(format!("Failed to run `{build_cmd_str}`: {e:#}")),
        };

        let mut result = format!(
            "Build command: {build_cmd_str}{release_flag}\nExit code: {}\n\nstdout:\n{}\n\nstderr:\n{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            release_flag = if release { " --release" } else { "" },
        );

        if output.status.success() {
            // Scan for output .wasm files so callers can pass the path directly to wavs_upload_component.
            // Check both wasip1 (cargo component) and wasip2 (standalone) output dirs.
            for target_dir in &[
                "target/wasm32-wasip1/release",
                "target/wasm32-wasip2/release",
            ] {
                let wasm_dir = dir_path.join(target_dir);
                if let Ok(entries) = std::fs::read_dir(&wasm_dir) {
                    let mut wasm_files: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("wasm"))
                        .filter_map(|p| p.to_str().map(|s| s.to_owned()))
                        .collect();
                    wasm_files.sort();
                    if !wasm_files.is_empty() {
                        result.push_str("\n\nOutput WASM files:");
                        for f in &wasm_files {
                            result.push_str(&format!("\n  {f}"));
                        }
                    }
                }
            }
            ok(result)
        } else {
            // Enhance error messages for common issues
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("failed to create a target world")
                || stderr.contains("package not found")
            {
                result.push_str(
                    "\n\n💡 Hint: WIT interface files may be missing or incomplete. \
                    For standalone projects, ensure all wit/deps/*/package.wit files are present. \
                    Re-run wavs_scaffold_component to get the complete file list.",
                );
            }
            if stderr.contains("no export") && stderr.contains("run") {
                result.push_str("\n\n💡 Hint: Component doesn't export the required 'run' function. \
                    Ensure the `export!()` macro (standalone) or `export_layer_trigger_world!()` macro (workspace) \
                    is present, and that `impl Guest for Component` is correct.");
            }
            err(result)
        }
    }

    async fn tool_validate_component(
        &self,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<CallToolResult, McpError> {
        let p: ValidateComponentParams = parse_args(args)?;
        let wasm_path = std::path::Path::new(&p.wasm_path);

        if !wasm_path.exists() {
            return err(format!("File not found: {}", p.wasm_path));
        }

        // Use wasm-tools to inspect the component
        let output = match tokio::process::Command::new("wasm-tools")
            .args(["component", "wit"])
            .arg(&p.wasm_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
        {
            Ok(o) => o,
            Err(e) => {
                return err(format!(
                    "Failed to run `wasm-tools component wit`: {e:#}\n\n\
                     Install with: cargo install wasm-tools"
                ))
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return err(format!(
                "❌ Not a valid WASI component.\n\n\
                 The file may be a core WebAssembly module (not a component).\n\
                 - If using standalone build, ensure you built with: `cargo build --target wasm32-wasip2 --release`\n\
                 - If using workspace build, ensure you used: `cargo component build --release`\n\n\
                 wasm-tools error:\n{stderr}"
            ));
        }

        let wit_output = String::from_utf8_lossy(&output.stdout);

        // Check for the required export
        let has_run_export = wit_output.contains("export run: func(trigger-action: trigger-action) -> result<list<wasm-response>, string>");

        // Check for key imports
        let has_operator_input = wit_output.contains("wavs:operator/input");
        let has_operator_output = wit_output.contains("wavs:operator/output");
        let has_types = wit_output.contains("wavs:types/");

        let mut issues: Vec<String> = Vec::new();
        let mut info: Vec<String> = Vec::new();

        if !has_run_export {
            issues.push(
                "Missing `export run` function. Ensure the export macro is present:\n  \
                 - Standalone: `bindings::export!(Component with_types_in bindings);`\n  \
                 - Workspace: `export_layer_trigger_world!(Component);`\n  \
                 And that `impl Guest for Component` has the correct signature."
                    .to_string(),
            );
        } else {
            info.push("✅ Exports `run` function with correct signature".to_string());
        }

        if !has_operator_input || !has_operator_output {
            issues.push(
                "Missing wavs:operator imports. The WIT files may be incomplete or corrupted."
                    .to_string(),
            );
        } else {
            info.push("✅ Imports wavs:operator input/output interfaces".to_string());
        }

        if !has_types {
            issues.push(
                "Missing wavs:types imports. Ensure wavs-types WIT dep is present.".to_string(),
            );
        } else {
            info.push("✅ Imports wavs:types definitions".to_string());
        }

        // File size info
        if let Ok(metadata) = std::fs::metadata(&p.wasm_path) {
            let size_kb = metadata.len() / 1024;
            info.push(format!("📦 Component size: {} KB", size_kb));
        }

        let mut result = String::new();
        if issues.is_empty() {
            result.push_str("# ✅ Component Validation Passed\n\n");
            result.push_str(&format!("File: `{}`\n\n", p.wasm_path));
            for line in &info {
                result.push_str(&format!("{line}\n"));
            }
            result.push_str("\nThe component is ready for upload with `wavs_upload_component`.");
            ok(result)
        } else {
            result.push_str("# ❌ Component Validation Failed\n\n");
            result.push_str(&format!("File: `{}`\n\n", p.wasm_path));
            for line in &info {
                result.push_str(&format!("{line}\n"));
            }
            result.push_str("\n## Issues\n\n");
            for issue in &issues {
                result.push_str(&format!("- {issue}\n\n"));
            }
            err(result)
        }
    }

    fn tool_get_service_schema(&self) -> Result<CallToolResult, McpError> {
        ok(r#"## Service JSON Schema

Use this as a reference when calling wavs_save_service or wavs_deploy_dev_service.

### Digest format
Raw 64-character hex string returned by wavs_upload_component. NO "sha256:" prefix.
Example: f0b42a5171c9dcd75eac41c8ce2c4e7882d304c885266d8ac7b70af996b9a420

### Manager address
The `manager` field uniquely identifies the service.
- For real deployments: use the actual on-chain ServiceManager contract address.
- For dev/testing (wavs_deploy_dev_service): use any placeholder (e.g. `0x1234...`). 
  The tool automatically replaces placeholder addresses with unique random ones to avoid collisions.

---

### Manual trigger (fires only via wavs_simulate_trigger)
```json
{
  "name": "my-service",
  "status": "active",
  "manager": {"evm": {"chain": "evm:31337", "address": "0x1234567890abcdef1234567890abcdef12345678"}},
  "workflows": {
    "default": {
      "trigger": "manual",
      "component": {
        "source": {"digest": "<64-char hex from wavs_upload_component>"},
        "permissions": {"file_system": false, "allowed_http_hosts": "none", "raw_sockets": false, "dns_resolution": false},
        "fuel_limit": null,
        "time_limit_seconds": null,
        "config": {},
        "env_keys": []
      },
      "submit": "none"
    }
  }
}
```

### Cron trigger (7-field: sec min hour dom month dow year)
```json
{
  "name": "my-cron-service",
  "status": "active",
  "manager": {"evm": {"chain": "evm:31337", "address": "0x1234567890abcdef1234567890abcdef12345678"}},
  "workflows": {
    "default": {
      "trigger": {"cron": {"schedule": "0 * * * * * *", "start_time": null, "end_time": null}},
      "component": {
        "source": {"digest": "<64-char hex>"},
        "permissions": {"file_system": false, "allowed_http_hosts": "none", "raw_sockets": false, "dns_resolution": false},
        "fuel_limit": null,
        "time_limit_seconds": null,
        "config": {},
        "env_keys": []
      },
      "submit": "none"
    }
  }
}
```

### Block interval trigger
```json
{
  "trigger": {"block_interval": {"chain": "evm:31337", "interval": 10}}
}
```

### EVM contract event trigger
```json
{
  "trigger": {
    "evm_contract_event": {
      "chain": "evm:31337",
      "address": "0xTriggerContractAddress",
      "event_hash": "0x<32-byte-keccak-of-event-signature>"
    }
  }
}
```

### Cosmos contract event trigger
```json
{
  "trigger": {
    "cosmos_contract_event": {
      "chain": "cosmos:mychain",
      "address": "cosmos1contract...",
      "event_type": "wasm-my-event"
    }
  }
}
```

---

### Submit options

**Discard output (most common for dev/testing):**
```json
"submit": "none"
```

**Submit on-chain via aggregator (requires simple-aggregator component):**
```json
"submit": {
  "aggregator": {
    "component": {
      "source": {"digest": "<digest of simple-aggregator.wasm>"},
      "permissions": {"file_system": false, "allowed_http_hosts": "none", "raw_sockets": false, "dns_resolution": false},
      "fuel_limit": null,
      "time_limit_seconds": null,
      "config": {
        "chain": "evm:31337",
        "service_handler": "0xReceiverContract"
      },
      "env_keys": []
    },
    "signature_kind": {
      "algorithm": "secp256k1",
      "prefix": "eip191"
    }
  }
}
```
IMPORTANT: when using aggregator submit, you must upload simple-aggregator.wasm as a SECOND component
and use its digest in the submit.aggregator.component.source.digest field above.
The receiver contract address goes in component.config["service_handler"] and the chain key in component.config["chain"].
There is NO top-level "contract", "quorum_percent", or "allowed_operators" field — those do not exist.

---

### SimulateTrigger data_json formats

Manual:        {"Raw": [104, 101, 108, 108, 111]}  (byte array)
Cron:          {"Cron": {"trigger_time": 1700000000}}
Block:         {"BlockInterval": {"block_height": 42}}
EVM event:     {"EvmContractEvent": {"chain": "evm:31337", "contract_address": "0x<contract>", "log_data": {"topics": ["0x<event-sig-hash>"], "data": "0x"}, "tx_hash": "0x<tx-hash>", "block_number": 12, "log_index": 0, "block_hash": "0x<block-hash>", "block_timestamp": null, "tx_index": 0}}
Cosmos event:  {"CosmosContractEvent": {"contract_address": "cosmos1...", "chain": "cosmos:mychain", "event": {"ty": "wasm-my-event", "attributes": []}, "block_height": 100, "event_index": 0}}

Note: trigger_json for simulate uses {"manual": null}, not the bare string "manual".
"#)
    }
}

// ── ServerHandler ──────────────────────────────────────────────────────────

impl ServerHandler for WavsMcpServer {
    fn get_info(&self) -> ServerInfo {
        let mut instructions = String::from(
            "MCP server for the WAVS (WebAssembly-based Actively Validated Services) platform.\n\
             \n\
             Read tools (no auth needed): wavs_get_node_info, wavs_get_health, wavs_list_services, wavs_get_service\n\
             Write tools (need --token): wavs_deploy_service, wavs_delete_service\n\
             Dev tools (need dev endpoints): wavs_upload_component, wavs_save_service, wavs_simulate_trigger, wavs_deploy_dev_service, wavs_query_kv\n\
             Chain-write tools (need WAVS_MCP_CHAIN_CREDENTIAL on MCP server): wavs_set_service_uri, wavs_deploy_service_manager, wavs_deploy_poa_service_manager\n\
             Chain-write tools (also need WAVS_SIGNING_MNEMONIC): wavs_register_operator, wavs_deploy_and_register, wavs_get_signing_address\n\
             Node-read tools (need --token): wavs_get_service_signer\n\
             Local tools: wavs_get_service_schema, wavs_get_wit_interface, wavs_scaffold_component, wavs_build_component, wavs_validate_component",
        );
        if self.exec_enabled {
            instructions.push_str(
                "\n\nExecution tools (--exec-enabled): wavs_exec_* tools are dynamically generated \
                 for each deployed service workflow. Use trust_tier to select result_only, signed_result, \
                 or on_chain execution mode.",
            );
        }
        ServerInfo {
            server_info: Implementation {
                name: "wavs-mcp".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(true),
                }),
                ..Default::default()
            },
            instructions: Some(instructions),
            ..Default::default()
        }
    }

    fn set_peer(&mut self, peer: Peer<RoleServer>) {
        let peer_store = self.peer.clone();
        tokio::spawn(async move {
            *peer_store.write().await = Some(peer);
        });
    }

    fn get_peer(&self) -> Option<Peer<RoleServer>> {
        self.peer.try_read().ok().and_then(|g| g.clone())
    }

    async fn list_tools(
        &self,
        _req: PaginatedRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let empty = no_params();

        let mut tools = vec![
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
                    description: "Production workflow: registers a service whose URI is already set on-chain \
                        (call wavs_save_service + wavs_set_service_uri first). \
                        For dev/testing without an on-chain contract, use wavs_deploy_dev_service instead. \
                        Pass service_manager_json as: \
                        {\"evm\":{\"chain\":\"evm:31337\",\"address\":\"0x...\"}} or \
                        {\"cosmos\":{\"chain\":\"cosmos:mychain\",\"address\":\"cosmos1...\"}}. Requires --token.".into(),
                    input_schema: schema_for_type::<ServiceManagerParams>().into(),
                },
                Tool {
                    name: "wavs_delete_service".into(),
                    description: "Delete a registered service. Requires --token.".into(),
                    input_schema: schema_for_type::<ServiceManagerParams>().into(),
                },
                // Chain-write tools (need WAVS_MCP_CHAIN_CREDENTIAL on MCP server)
                Tool {
                    name: "wavs_set_service_uri".into(),
                    description: "Call setServiceURI on the ServiceManager contract to update the \
                        on-chain service URI. Requires --mcp-chain-credential (WAVS_MCP_CHAIN_CREDENTIAL) \
                        to be configured on this MCP server. Provide the chain RPC URL as rpc_url. \
                        EVM only currently.".into(),
                    input_schema: schema_for_type::<SetServiceUriParams>().into(),
                },
                Tool {
                    name: "wavs_deploy_service_manager".into(),
                    description: "Deploy a new SimpleServiceManager PoA contract on-chain and return its address. \
                        Requires --mcp-chain-credential (WAVS_MCP_CHAIN_CREDENTIAL) on this MCP server. \
                        Provide the chain RPC URL as rpc_url. EVM only currently.".into(),
                    input_schema: schema_for_type::<DeployServiceManagerParams>().into(),
                },
                Tool {
                    name: "wavs_deploy_poa_service_manager".into(),
                    description: "Deploy a new POAStakeRegistry (full PoA middleware with proxy) on-chain via Docker. \
                        Returns the proxy address to use as service manager. \
                        Requires --mcp-chain-credential on this MCP server. \
                        Docker image ghcr.io/lay3rlabs/poa-middleware:1.0.1 must be available. \
                        Provide the chain RPC URL as rpc_url. EVM only currently. \
                        After deploying, upload+save+deploy the service first, then call wavs_register_operator \
                        so the node has assigned a signing key that can be registered on-chain.".into(),
                    input_schema: schema_for_type::<DeployPoaServiceManagerParams>().into(),
                },
                Tool {
                    name: "wavs_register_operator".into(),
                    description: "PoA setup: registers the WAVS node's signing key as an operator on a POAStakeRegistry \
                        contract and sets the signing key mapping. IMPORTANT: call this AFTER wavs_deploy_service (or \
                        wavs_deploy_dev_service) — it queries the WAVS node for the service-specific HD-derived signing \
                        key (the key the node actually uses to sign envelopes) and registers that address on-chain. \
                        Calls registerOperator (using WAVS_MCP_CHAIN_CREDENTIAL as owner) and \
                        updateOperatorSigningKey (using WAVS_SIGNING_MNEMONIC as operator). \
                        weight is a relative stake weight (default: 100; any positive value works for single-operator setups). \
                        Requires --mcp-chain-credential and --signing-mnemonic on this MCP server. \
                        Provide the chain RPC URL as rpc_url. EVM only currently.".into(),
                    input_schema: schema_for_type::<RegisterOperatorParams>().into(),
                },
                Tool {
                    name: "wavs_deploy_and_register".into(),
                    description: "POA convenience: atomically deploys a service to the WAVS node AND registers the \
                        operator on the POAStakeRegistry in one call. Equivalent to wavs_deploy_service followed by \
                        wavs_register_operator. The service must already be saved and its URI set on-chain \
                        (run wavs_set_service_uri first). Requires --token, --mcp-chain-credential, and \
                        --signing-mnemonic. EVM only.".into(),
                    input_schema: schema_for_type::<DeployAndRegisterParams>().into(),
                },
                Tool {
                    name: "wavs_get_service_signer".into(),
                    description: "Query the WAVS node for the HD-derived signing key assigned to a specific service. \
                        Returns the HD index and EVM address the node uses to sign envelopes for that service. \
                        Useful for diagnosing POAStakeRegistry InvalidSignature errors and verifying \
                        wavs_register_operator registered the correct key. Requires --token.".into(),
                    input_schema: schema_for_type::<ServiceManagerParams>().into(),
                },
                Tool {
                    name: "wavs_get_signing_address".into(),
                    description: "Derive the EVM address for any HD index of the WAVS signing mnemonic without \
                        network access. Defaults to HD index 0 (the operator identity). Pass hd_index to check \
                        a service-specific key (use the index from wavs_get_service_signer). \
                        Requires --signing-mnemonic (WAVS_SIGNING_MNEMONIC) to be configured on this MCP server.".into(),
                    input_schema: schema_for_type::<GetSigningAddressParams>().into(),
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
                        Requires dev endpoints enabled in wavs.toml. \
                        Call wavs_get_service_schema first to see a minimal valid example.".into(),
                    input_schema: schema_for_type::<SaveServiceParams>().into(),
                },
                Tool {
                    name: "wavs_simulate_trigger".into(),
                    description: "Simulate a trigger against a deployed service. \
                        The service_id parameter is the 64-char hex ID returned by wavs_deploy_dev_service \
                        (labeled as `service_id`, NOT the `deploy_hash`). \
                        The trigger_json and data_json must match the trigger type configured in the service. \
                        Use wavs_get_service_schema for examples of trigger/data JSON formats. \
                        Requires dev endpoints enabled in wavs.toml.".into(),
                    input_schema: schema_for_type::<SimulateTriggerParams>().into(),
                },
                Tool {
                    name: "wavs_deploy_dev_service".into(),
                    description: "Register a service directly without an on-chain contract (dev/testing only). \
                        Pass the full Service JSON. Placeholder manager addresses (like 0x1234...) are \
                        automatically replaced with unique random addresses to prevent collisions. \
                        Returns the service_id (needed for wavs_simulate_trigger) and other details. \
                        Handles the two-step save+register flow internally. \
                        Requires dev endpoints enabled in wavs.toml and --token. \
                        Call wavs_get_service_schema first to see a minimal valid example. \
                        Use this for local dev. For production with a real ServiceManager contract, \
                        use wavs_save_service → wavs_set_service_uri → wavs_deploy_service instead.".into(),
                    input_schema: schema_for_type::<DeployDevServiceParams>().into(),
                },
                Tool {
                    name: "wavs_query_kv".into(),
                    description: "Read a value from a service's KV store. \
                        Useful for inspecting state written by kv-store components. \
                        Requires dev endpoints enabled in wavs.toml.".into(),
                    input_schema: schema_for_type::<QueryKvParams>().into(),
                },
                Tool {
                    name: "wavs_query_logs".into(),
                    description: "Query structured log entries from the WAVS node's in-memory ring buffer. \
                        Returns a JSON object with `entries` and `next_id`. \
                        Pass the returned `next_id` as `since_id` on subsequent calls to receive only new entries. \
                        For WASM component execution logs use `wavs_query_component_logs` instead. \
                        Requires dev endpoints enabled in wavs.toml.".into(),
                    input_schema: schema_for_type::<QueryLogsParams>().into(),
                },
                Tool {
                    name: "wavs_query_component_logs".into(),
                    description: "Query logs emitted by WASM components during operator/aggregator execution. \
                        Filters automatically to component logs and supports narrowing by `service_id`, \
                        `workflow_id`, and `digest`. Each entry's `fields` contains the component message \
                        plus those identifiers. Returns a JSON object with `entries` and `next_id`; \
                        pass `next_id` as `since_id` to page forward. \
                        Requires dev endpoints enabled in wavs.toml.".into(),
                    input_schema: schema_for_type::<QueryComponentLogsParams>().into(),
                },
                // Local tools
                tool("wavs_get_service_schema",
                     "Return minimal valid Service JSON examples for every trigger type \
                      (manual, cron, block_interval, evm_contract_event, cosmos_contract_event), \
                      submit options (none vs aggregator), and data_json formats for wavs_simulate_trigger. \
                      Call this before wavs_save_service or wavs_deploy_dev_service to avoid schema errors.",
                     empty.clone()),
                tool("wavs_get_wit_interface",
                     "Return the full WIT interface definitions for WAVS WASM components \
                      (HTTP, KV, sockets, TLS, host functions, etc.)",
                     empty.clone()),
                Tool {
                    name: "wavs_scaffold_component".into(),
                    description: "Create a complete, ready-to-build WAVS WASM component project. \
                        If `dir` is provided, writes all files to disk at `{dir}/{name}/` (recommended). \
                        If `dir` is omitted, returns file contents as text for manual creation. \
                        Includes Cargo.toml, src/lib.rs, src/bindings.rs, and the full WIT interface directory. \
                        The generated project is self-contained and builds with `cargo build --target wasm32-wasip2 --release`. \
                        After scaffolding, customize src/lib.rs then use wavs_build_component to compile. \
                        Trigger types: evm_contract_event | cosmos_contract_event | block_interval | cron | manual".into(),
                    input_schema: schema_for_type::<ScaffoldComponentParams>().into(),
                },
                Tool {
                    name: "wavs_build_component".into(),
                    description: "Build a WAVS WASM component. \
                        Auto-detects build mode: uses `cargo build --target wasm32-wasip2` for standalone projects \
                        (with local wit/ directory) or `cargo component build` for workspace projects. \
                        Returns full build output and output .wasm file paths.".into(),
                    input_schema: schema_for_type::<BuildComponentParams>().into(),
                },
                Tool {
                    name: "wavs_validate_component".into(),
                    description: "Validate a compiled .wasm component before uploading. \
                        Checks that the file is a valid WASI component (not a core module), \
                        exports the required `run` function with the correct signature, \
                        and imports the expected WAVS interfaces. \
                        Requires `wasm-tools` to be installed. \
                        Run this after wavs_build_component and before wavs_upload_component.".into(),
                    input_schema: schema_for_type::<ValidateComponentParams>().into(),
                },
            ];

        // Conditionally add dynamic exec tools for deployed services
        if self.exec_enabled {
            match self.get_services_cached().await {
                Ok(services) => {
                    let exec_tools = exec::build_exec_tools(&services);
                    tools.extend(exec_tools);
                }
                Err(e) => {
                    tracing::warn!("Failed to build exec tools: {}", e.message);
                    // Continue with just management tools -- don't fail the whole list
                }
            }
        }

        Ok(ListToolsResult {
            tools,
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
            "wavs_get_node_info" => self.tool_get_node_info().await,
            "wavs_get_health" => self.tool_get_health().await,
            "wavs_list_services" => self.tool_list_services().await,
            "wavs_get_service" => self.tool_get_service(args).await,
            "wavs_deploy_service" => self.tool_deploy_service(args).await,
            "wavs_delete_service" => self.tool_delete_service(args).await,
            "wavs_set_service_uri" => self.tool_set_service_uri(args).await,
            "wavs_deploy_service_manager" => self.tool_deploy_service_manager(args).await,
            "wavs_deploy_poa_service_manager" => self.tool_deploy_poa_service_manager(args).await,
            "wavs_register_operator" => self.tool_register_operator(args).await,
            "wavs_deploy_and_register" => self.tool_deploy_and_register(args).await,
            "wavs_get_service_signer" => self.tool_get_service_signer(args).await,
            "wavs_get_signing_address" => self.tool_get_signing_address(args).await,
            "wavs_upload_component" => self.tool_upload_component(args).await,
            "wavs_save_service" => self.tool_save_service(args).await,
            "wavs_simulate_trigger" => self.tool_simulate_trigger(args).await,
            "wavs_deploy_dev_service" => self.tool_deploy_dev_service(args).await,
            "wavs_query_kv" => self.tool_query_kv(args).await,
            "wavs_query_logs" => self.tool_query_logs(args).await,
            "wavs_query_component_logs" => self.tool_query_component_logs(args).await,
            "wavs_get_service_schema" => self.tool_get_service_schema(),
            "wavs_get_wit_interface" => self.tool_get_wit_interface().await,
            "wavs_scaffold_component" => self.tool_scaffold_component(args).await,
            "wavs_build_component" => self.tool_build_component(args).await,
            "wavs_validate_component" => self.tool_validate_component(args).await,
            name if name.starts_with("wavs_exec_") => {
                if !self.exec_enabled {
                    return Err(ErrorData {
                        code: ErrorCode::INVALID_REQUEST,
                        message: "Execution tools are disabled. Restart the MCP server with --exec-enabled.".into(),
                        data: None,
                    });
                }
                let services = self.get_services_cached().await?;
                let signing_cred = self
                    .signing_mnemonic
                    .as_deref()
                    .and_then(|s| s.parse::<wavs_types::Credential>().ok());
                let chain_cred = self
                    .mcp_chain_credential
                    .as_deref()
                    .and_then(|s| s.parse::<wavs_types::Credential>().ok());
                let ctx = exec::ExecContext {
                    client: &self.client,
                    services_json: &services,
                    signing_mnemonic: signing_cred.as_ref(),
                    mcp_chain_credential: chain_cred.as_ref(),
                    pending_confirmations: Some(&self.pending_confirmations),
                };
                exec::handle_exec_tool(&ctx, name, args).await
            }
            name => Err(ErrorData {
                code: ErrorCode::METHOD_NOT_FOUND,
                message: format!("Unknown tool: {name}").into(),
                data: None,
            }),
        }
    }
}
