use std::sync::Arc;

use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use wavs_gui_shared::error::{AppError, AppResult};
use wavs_gui_shared::event::{
    AgentRpcEvent, AgentStatusEvent, AgentUiControlEvent, TauriEventEmitterExt,
};

struct PiSidecarInner {
    child: Child,
    stdin_tx: tokio::sync::mpsc::Sender<String>,
    relay_handle: tokio::task::JoinHandle<()>,
    stdin_handle: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
pub struct PiSidecarState {
    inner: Arc<Mutex<Option<PiSidecarInner>>>,
}

impl PiSidecarState {
    pub async fn start(&self, app: AppHandle, config: PiSidecarConfig) -> AppResult<()> {
        // Kill existing if running
        self.stop(&app).await?;

        let mut cmd = Command::new("npx");
        cmd.arg("tsx")
            .arg(&config.entrypoint_path)
            .current_dir(&config.agent_package_dir)
            .env("WAVS_URL", &config.wavs_url)
            .env("WAVS_MCP_TOKEN", config.mcp_token.as_deref().unwrap_or(""))
            .env("WAVS_HOME", &config.wavs_home)
            .env("WAVS_AGENT_WORKSPACE", &config.workspace_dir)
            .env("WAVS_AUTH_DIR", &config.auth_dir);
        if let Some(ref mcp_bin) = config.mcp_binary_path {
            cmd.env("WAVS_MCP_BINARY", mcp_bin);
        }
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Agent(format!("Failed to spawn pi sidecar: {}", e)))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::Agent("No stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Agent("No stdout".into()))?;

        // Channel for writing to stdin — both send_command and the relay can use this
        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel::<String>(64);

        // Stdin writer task
        let stdin_handle = tokio::spawn(async move {
            while let Some(cmd) = stdin_rx.recv().await {
                if stdin.write_all(cmd.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                if stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        // Clone stdin_tx for the relay to use
        let relay_stdin_tx = stdin_tx.clone();

        // Spawn stdout relay task — reads JSON lines from pi and emits Tauri events
        let app_clone = app.clone();
        let relay_handle = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    if json.get("type").and_then(|t| t.as_str()) == Some("response") {
                        if json.get("success").and_then(|s| s.as_bool()) == Some(false) {
                            tracing::warn!("RPC command failed: {}", line);
                        }
                        let cmd_name = json.get("command").and_then(|c| c.as_str()).unwrap_or("");

                        // When switch_session completes, automatically request messages
                        if cmd_name == "switch_session" {
                            if json.get("success").and_then(|s| s.as_bool()) == Some(true) {
                                tracing::info!("Session switched, requesting messages");
                                let get_msg_cmd = serde_json::json!({"type": "get_messages"});
                                let _ = relay_stdin_tx.send(get_msg_cmd.to_string()).await;
                            }
                            continue;
                        }

                        // Forward get_messages responses as session_messages events
                        if cmd_name == "get_messages" {
                            if let Some(data) = json.get("data") {
                                if let Some(messages) = data.get("messages") {
                                    let msg_count = messages.as_array().map(|a| a.len()).unwrap_or(0);
                                    tracing::info!("Forwarding session_messages with {} messages", msg_count);
                                    let event = serde_json::json!({
                                        "type": "session_messages",
                                        "messages": messages,
                                    });
                                    let _ = app_clone.emit_ext(AgentRpcEvent { event });
                                }
                            }
                            continue;
                        }

                        // Skip other responses
                        continue;
                    }
                    if is_ui_control_event(&json) {
                        handle_ui_control(&app_clone, &json);
                    }
                    // Always forward to frontend (including ui_control events, so tool status updates)
                    let _ = app_clone.emit_ext(AgentRpcEvent { event: json });
                }
            }
            // Process ended
            let _ = app_clone.emit_ext(AgentStatusEvent {
                status: "stopped".into(),
                error: Some("Agent process exited".into()),
            });
        });

        // Spawn stderr reader (log to tracing)
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::info!(target: "pi_sidecar", "{}", line);
                }
            });
        }

        *self.inner.lock().await = Some(PiSidecarInner {
            child,
            stdin_tx,
            relay_handle,
            stdin_handle,
        });

        let _ = app.emit_ext(AgentStatusEvent {
            status: "running".into(),
            error: None,
        });

        Ok(())
    }

    pub async fn stop(&self, app: &AppHandle) -> AppResult<()> {
        let mut guard = self.inner.lock().await;
        if let Some(mut inner) = guard.take() {
            inner.relay_handle.abort();
            inner.stdin_handle.abort();
            let _ = inner.child.kill().await;
            let _ = app.emit_ext(AgentStatusEvent {
                status: "stopped".into(),
                error: None,
            });
        }
        Ok(())
    }

    pub async fn is_running(&self) -> bool {
        self.inner.lock().await.is_some()
    }

    pub async fn send_command(&self, command: &str) -> AppResult<()> {
        let guard = self.inner.lock().await;
        if let Some(inner) = guard.as_ref() {
            inner
                .stdin_tx
                .send(command.to_string())
                .await
                .map_err(|e| AppError::Agent(format!("Failed to send command: {}", e)))?;
            Ok(())
        } else {
            Err(AppError::Agent("Agent not running".into()))
        }
    }
}

pub struct PiSidecarConfig {
    pub entrypoint_path: String,
    pub agent_package_dir: String,
    pub wavs_url: String,
    pub mcp_token: Option<String>,
    pub wavs_home: String,
    pub workspace_dir: String,
    pub auth_dir: String,
    pub mcp_binary_path: Option<String>,
}

/// Check if the event is a UI control event from the __ui_control extension tool.
fn is_ui_control_event(json: &serde_json::Value) -> bool {
    if json.get("type").and_then(|t| t.as_str()) != Some("tool_execution_end") {
        return false;
    }
    json.get("toolName")
        .and_then(|n| n.as_str())
        .map(|n| n.starts_with("ui_"))
        .unwrap_or(false)
}

/// Handle a UI control event by parsing and emitting it as an AgentUiControlEvent.
fn handle_ui_control(app: &AppHandle, json: &serde_json::Value) {
    // The tool result has `result.details` with `{ action, path/message/level/... }`
    let details = json
        .get("result")
        .and_then(|r| r.get("details"))
        .cloned()
        .unwrap_or_default();
    let action = details
        .get("action")
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();
    tracing::info!("UI control event: action={}, details={}", action, details);
    let _ = app.emit_ext(AgentUiControlEvent {
        action,
        payload: details,
    });
}
