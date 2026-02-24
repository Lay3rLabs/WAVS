use std::sync::Arc;

use rmcp::{
    ServerHandler,
    handler::server::tool::schema_for_type,
    model::*,
    schemars,
    service::{RequestContext, RoleServer},
};
use serde::Deserialize;

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
    /// Trigger definition as JSON, e.g. `{"cron":{"schedule":"* * * * *","start_time":null,"end_time":null}}`
    pub trigger_json: String,
    /// TriggerData as JSON, e.g. `{"Cron":{"trigger_time":0}}`
    pub data_json: String,
    /// How many times to fire the trigger (default: 1)
    pub count: Option<usize>,
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
}

impl WavsMcpServer {
    pub fn new(wavs_url: String, token: Option<String>) -> Self {
        Self {
            client: WavsClient::new(wavs_url, token),
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
                 Dev tools (need dev endpoints): wavs_upload_component, wavs_simulate_trigger, wavs_deploy_dev_service, wavs_query_kv\n\
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
                // Dev tools
                Tool {
                    name: "wavs_upload_component".into(),
                    description: "Upload a compiled .wasm binary to the WAVS node. Returns the component digest. \
                        Requires dev endpoints enabled in wavs.toml.".into(),
                    input_schema: schema_for_type::<UploadComponentParams>().into(),
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
            "wavs_upload_component"      => self.tool_upload_component(args).await,
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
