//! Subprocess runner — spawns the real `wavs` binary as a child process.
//!
//! The subprocess tier exists for tests that need every dispatcher subsystem in
//! the loop: real trigger streams, libp2p submission gossip, the actual HTTP
//! server, and the production WASM execution path. Boot the binary, register a
//! service, drive triggers via on-chain emission, wait for the submission tx,
//! and assert on contract state.
//!
//! The in-process tier ([`super::InProcRunner`]) remains canonical for fast PR
//! tests; this tier is for nightly fork matrices and pre-release verification.
//!
//! ## What v1 ships
//!
//! - [`SubprocessConfig`] builder for the spawn inputs.
//! - [`SubprocessRunner::start`] — generates a minimal `wavs.toml`, picks a
//!   random port, spawns the binary with `--dev-endpoints-enabled true`, and
//!   polls `/health` until the server is up.
//! - [`SubprocessRunner::register_service`] — `POST /dev/services` with a JSON
//!   service definition.
//! - [`SubprocessRunner::http_base`] — base URL `http://127.0.0.1:<port>` for
//!   tests that need to drive other HTTP endpoints directly.
//! - Clean shutdown on [`SubprocessRunner::shutdown`] or `Drop`: SIGINT, then
//!   wait, then SIGKILL if the child doesn't exit.
//!
//! Locating the `wavs` binary: set `SubprocessConfig::wavs_binary(path)`
//! explicitly, or set the `WAVS_BINARY` env var, or place a built binary at
//! `<WAVS-repo>/target/{debug,release}/wavs` (the runner probes both).
//!
//! ## Follow-ups (deferred for the next PR)
//!
//! - Trigger emission and quorum waiting via the harness's chain primitives
//!   (works today — just compose `chain::*` with this runner).
//! - Bundled `wavs.toml` chain configs for the common Anvil/fork scenarios.
//! - Multi-operator P2P quorum tests (layer-tests still covers that path
//!   in-process today).

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use tokio::{io::AsyncReadExt, process::Child, task::JoinHandle};

use crate::service::ServiceSpec;

/// Configuration for the subprocess WAVS runner.
#[derive(Debug, Clone, Default)]
pub struct SubprocessConfig {
    wavs_binary: Option<PathBuf>,
    rpc_url: Option<String>,
    data_dir: Option<PathBuf>,
    home_dir: Option<PathBuf>,
    port: Option<u16>,
    extra_env: Vec<(String, String)>,
    extra_args: Vec<String>,
    startup_timeout: Option<Duration>,
    signing_mnemonic: Option<String>,
}

impl SubprocessConfig {
    /// Empty config — fill via the builder methods.
    pub fn new() -> Self {
        Self::default()
    }

    /// Path to the `wavs` binary. If unset, the runner probes
    /// `WAVS_BINARY` env, then `target/{debug,release}/wavs` under
    /// `CARGO_WORKSPACE_DIR` (or `..` of CARGO_MANIFEST_DIR).
    pub fn wavs_binary(mut self, path: impl AsRef<Path>) -> Self {
        self.wavs_binary = Some(path.as_ref().to_path_buf());
        self
    }

    /// EVM RPC URL the WAVS node connects to (Anvil endpoint or fork URL).
    /// Wired into the generated `wavs.toml` as the default chain.
    pub fn rpc_url(mut self, url: impl Into<String>) -> Self {
        self.rpc_url = Some(url.into());
        self
    }

    /// Directory the WAVS node uses for `WAVS_DATA`. Defaults to a tempdir.
    pub fn data_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.data_dir = Some(path.as_ref().to_path_buf());
        self
    }

    /// Directory the WAVS node uses for `WAVS_HOME`. Defaults to a tempdir
    /// containing a generated `wavs.toml`.
    pub fn home_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.home_dir = Some(path.as_ref().to_path_buf());
        self
    }

    /// Bind port for the HTTP server. If unset the OS picks one.
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Add a `(KEY, VALUE)` pair to the spawned process environment.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_env.push((key.into(), value.into()));
        self
    }

    /// Append an extra CLI flag the runner will pass to the binary.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    /// Override the startup health-probe timeout. Defaults to 30 seconds.
    pub fn startup_timeout(mut self, d: Duration) -> Self {
        self.startup_timeout = Some(d);
        self
    }

    /// Override the BIP-39 mnemonic used for the operator signing key.
    /// Defaults to the canonical test mnemonic ("test test test … junk").
    pub fn signing_mnemonic(mut self, m: impl Into<String>) -> Self {
        self.signing_mnemonic = Some(m.into());
        self
    }

    pub fn wavs_binary_path(&self) -> Option<&Path> {
        self.wavs_binary.as_deref()
    }

    pub fn rpc_url_value(&self) -> Option<&str> {
        self.rpc_url.as_deref()
    }

    pub fn data_dir_value(&self) -> Option<&Path> {
        self.data_dir.as_deref()
    }

    pub fn extra_env_pairs(&self) -> &[(String, String)] {
        &self.extra_env
    }
}

/// Subprocess runner handle. `Drop` reaps the child cleanly.
pub struct SubprocessRunner {
    config: SubprocessConfig,
    #[allow(dead_code)]
    spec: ServiceSpec,
    child: Option<Child>,
    port: u16,
    // Tempdir handles — must outlive the child.
    _home_dir: Option<tempfile::TempDir>,
    _data_dir: Option<tempfile::TempDir>,
    http: reqwest::Client,
    log_forwarders: Vec<JoinHandle<()>>,
}

impl SubprocessRunner {
    /// Build a subprocess runner from a config and service spec.
    ///
    /// The spec is validated lazily — at [`Self::register_service`] time, not
    /// here — so tests that only need to spawn the binary and probe `/health`
    /// can pass [`ServiceSpec::new()`] without populating WASM paths.
    pub fn new(config: SubprocessConfig, spec: ServiceSpec) -> Result<Self> {
        Ok(Self {
            config,
            spec,
            child: None,
            port: 0,
            _home_dir: None,
            _data_dir: None,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .context("build reqwest client")?,
            log_forwarders: Vec::new(),
        })
    }

    /// HTTP base URL once the binary is running. Empty until [`Self::start`].
    pub fn http_base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Start the WAVS subprocess and wait for `/health` to respond.
    ///
    /// Returns the bound HTTP base URL.
    pub async fn start(&mut self) -> Result<String> {
        // 1. Pick a port.
        let port = match self.config.port {
            Some(p) => p,
            None => pick_random_port()?,
        };
        self.port = port;

        // 2. Resolve binary path.
        let bin = resolve_wavs_binary(self.config.wavs_binary.as_deref())?;
        tracing::info!(binary = %bin.display(), port, "spawning wavs subprocess");

        // 3. Prepare home + data dirs. Either user-supplied (path) or tempdir.
        let (home_path, home_keep) = match self.config.home_dir.clone() {
            Some(p) => (p, None),
            None => {
                let td = tempfile::tempdir().context("home tempdir")?;
                (td.path().to_path_buf(), Some(td))
            }
        };
        let (data_path, data_keep) = match self.config.data_dir.clone() {
            Some(p) => (p, None),
            None => {
                let td = tempfile::tempdir().context("data tempdir")?;
                (td.path().to_path_buf(), Some(td))
            }
        };

        // 4. Write a minimal wavs.toml into HOME. The binary needs *some*
        //    config file or the chain registry stays empty; we wire in
        //    the user-supplied RPC URL if present.
        write_minimal_config(&home_path, self.config.rpc_url.as_deref())
            .context("write wavs.toml")?;

        // 5. Build argv.
        let mut cmd = tokio::process::Command::new(&bin);
        cmd.arg("--home")
            .arg(&home_path)
            .arg("--data")
            .arg(&data_path)
            .arg("--port")
            .arg(port.to_string())
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--dev-endpoints-enabled")
            .arg("true");

        let mnemonic = self
            .config
            .signing_mnemonic
            .clone()
            .unwrap_or_else(|| DEFAULT_TEST_MNEMONIC.to_string());
        cmd.env("WAVS_SIGNING_MNEMONIC", &mnemonic);

        for (k, v) in &self.config.extra_env {
            cmd.env(k, v);
        }
        for a in &self.config.extra_args {
            cmd.arg(a);
        }

        // Capture stdout/stderr so a failed boot surfaces in test output but
        // doesn't spew into the parent test runner unconditionally.
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn().with_context(|| {
            format!(
                "spawn wavs binary at {}; ensure it's built (`cargo build -p wavs`)",
                bin.display()
            )
        })?;

        // 6. Spawn a forwarder for stdout/stderr so we can collect log output
        //    on failure without losing it.
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let log_buf = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        if let Some(s) = stdout {
            self.log_forwarders
                .push(spawn_log_forwarder(s, log_buf.clone()));
        }
        if let Some(s) = stderr {
            self.log_forwarders
                .push(spawn_log_forwarder(s, log_buf.clone()));
        }

        self.child = Some(child);
        self._home_dir = home_keep;
        self._data_dir = data_keep;

        // 7. Poll /health until the server responds.
        let timeout = self.config.startup_timeout.unwrap_or(STARTUP_TIMEOUT);
        let deadline = Instant::now() + timeout;
        let base = self.http_base();
        let health = format!("{base}/health");
        loop {
            if let Ok(resp) = self.http.get(&health).send().await {
                if resp.status().is_success() {
                    tracing::info!(base = %base, "wavs subprocess ready");
                    return Ok(base);
                }
            }
            if Instant::now() > deadline {
                let logs = log_buf.lock().unwrap().clone();
                return Err(anyhow!(
                    "wavs subprocess did not become healthy within {:?}\n--- captured logs ---\n{}",
                    timeout,
                    logs
                ));
            }
            // If the child died early, fail fast with its output.
            if let Some(child) = self.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    let logs = log_buf.lock().unwrap().clone();
                    return Err(anyhow!(
                        "wavs subprocess exited early with {status}\n--- captured logs ---\n{logs}"
                    ));
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Register a service definition with the running node via the
    /// `POST /dev/services` endpoint. The body is the JSON serialization of
    /// [`wavs_types::Service`]; tests can build one via
    /// [`crate::service::InProcRunner::service`] (which returns a `&Service`
    /// the test can clone into the registration request) or by hand.
    ///
    /// Validates the stored `ServiceSpec` lazily so callers passing
    /// [`ServiceSpec::new()`] for spawn-only tests don't error here either —
    /// only the registration path needs the spec's wasm paths.
    pub async fn register_service(&self, body_json: &serde_json::Value) -> Result<String> {
        self.spec.validate().context("spec validation")?;
        let url = format!("{}/dev/services", self.http_base());
        let resp = self
            .http
            .post(&url)
            .json(body_json)
            .send()
            .await
            .context("POST /dev/services")?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("POST /dev/services failed: {status} body={body}"));
        }
        Ok(body)
    }

    /// Probe the `/health` endpoint and return its JSON response.
    pub async fn health(&self) -> Result<serde_json::Value> {
        let url = format!("{}/health", self.http_base());
        let resp = self.http.get(&url).send().await.context("GET /health")?;
        if !resp.status().is_success() {
            return Err(anyhow!("GET /health returned {}", resp.status()));
        }
        let json = resp.json().await.context("parse /health JSON")?;
        Ok(json)
    }

    /// SIGINT the child and wait for clean exit. Returns the exit status. On
    /// timeout, SIGKILLs the child.
    pub async fn shutdown(mut self) -> Result<std::process::ExitStatus> {
        self.shutdown_inner().await
    }

    async fn shutdown_inner(&mut self) -> Result<std::process::ExitStatus> {
        let Some(mut child) = self.child.take() else {
            return Err(anyhow!("subprocess not running"));
        };

        // Try SIGINT first.
        #[cfg(unix)]
        {
            if let Some(pid) = child.id() {
                let pid = i32::try_from(pid).unwrap_or(0);
                unsafe {
                    libc::kill(pid, libc::SIGINT);
                }
            }
        }

        let exit = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        let status = match exit {
            Ok(Ok(status)) => status,
            _ => {
                let _ = child.kill().await;
                child.wait().await.context("await sigkilled child")?
            }
        };
        self.join_log_forwarders().await;
        Ok(status)
    }
}

impl Drop for SubprocessRunner {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Best-effort cleanup; can't await in Drop.
            let _ = child.start_kill();
        }
        self.abort_log_forwarders();
    }
}

impl SubprocessRunner {
    async fn join_log_forwarders(&mut self) {
        for handle in self.log_forwarders.drain(..) {
            let _ = tokio::time::timeout(Duration::from_secs(1), handle).await;
        }
    }

    fn abort_log_forwarders(&mut self) {
        for handle in self.log_forwarders.drain(..) {
            handle.abort();
        }
    }
}

fn spawn_log_forwarder<R>(
    mut reader: R,
    buf: std::sync::Arc<std::sync::Mutex<String>>,
) -> JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut tmp = [0u8; 4096];
        while let Ok(n) = reader.read(&mut tmp).await {
            if n == 0 {
                break;
            }
            let chunk = String::from_utf8_lossy(&tmp[..n]).into_owned();
            buf.lock().unwrap().push_str(&chunk);
        }
    })
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

const DEFAULT_TEST_MNEMONIC: &str = "test test test test test test test test test test test junk";

fn pick_random_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").context("bind ephemeral port")?;
    Ok(listener.local_addr()?.port())
}

/// Exposed for integration tests that want to probe whether the binary is
/// available before attempting a spawn (and skip gracefully if not).
pub fn resolve_wavs_binary_for_tests() -> Result<PathBuf> {
    resolve_wavs_binary(None)
}

fn resolve_wavs_binary(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.exists() {
            return Err(anyhow!(
                "explicit wavs_binary path {} does not exist",
                p.display()
            ));
        }
        return Ok(p.to_path_buf());
    }
    if let Ok(p) = std::env::var("WAVS_BINARY") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        }
    }
    // Probe target/{debug,release}/wavs relative to the workspace root.
    // CARGO_MANIFEST_DIR points at this crate (`packages/test-harness`); the
    // workspace root is two levels up.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| anyhow!("could not derive workspace dir from CARGO_MANIFEST_DIR"))?;
    for profile in ["debug", "release"] {
        let probe = workspace.join("target").join(profile).join("wavs");
        if probe.exists() {
            return Ok(probe);
        }
    }
    Err(anyhow!(
        "could not locate wavs binary — set SubprocessConfig::wavs_binary(), \
         the WAVS_BINARY env var, or `cargo build -p wavs` first"
    ))
}

fn write_minimal_config(home: &Path, rpc_url: Option<&str>) -> Result<()> {
    let cfg_path = home.join("wavs.toml");
    let mut contents = String::from(
        "# Auto-generated by wavs-test-harness::SubprocessRunner.\n\
         # Minimal config — extend by writing your own wavs.toml at home_dir().\n\
         \n\
         [default]\n\
         log_level = [\"info\", \"wavs=debug\"]\n\
         \n\
         [wavs]\n\
         host = \"127.0.0.1\"\n\
         max_wasm_fuel = 1000000000\n\
         max_execution_seconds = 30\n\
         \n",
    );
    if let Some(url) = rpc_url {
        contents.push_str(&format!(
            "[wavs.chains.evm.\"evm:local\"]\nws_endpoint = \"{url}\"\nhttp_endpoint = \"{url}\"\npoll_interval_ms = 100\n\n"
        ));
    }
    std::fs::write(&cfg_path, contents).with_context(|| format!("write {}", cfg_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_builder_round_trip() {
        let c = SubprocessConfig::new()
            .wavs_binary("/usr/local/bin/wavs")
            .rpc_url("http://127.0.0.1:8545")
            .data_dir("/tmp/wavs-test")
            .env("WAVS_LOG", "debug");
        assert_eq!(
            c.wavs_binary_path()
                .map(|p| p.to_string_lossy().to_string()),
            Some("/usr/local/bin/wavs".to_string())
        );
        assert_eq!(c.rpc_url_value(), Some("http://127.0.0.1:8545"));
        assert_eq!(c.extra_env_pairs().len(), 1);
    }

    #[test]
    fn pick_random_port_works() {
        let p = pick_random_port().unwrap();
        assert!(p > 1024, "ephemeral port should be > 1024, got {p}");
    }

    #[test]
    fn resolve_wavs_binary_explicit_missing() {
        let res = resolve_wavs_binary(Some(Path::new("/tmp/definitely-not-here-xyz")));
        assert!(res.is_err());
    }

    #[test]
    fn write_minimal_config_includes_rpc_when_set() {
        let td = tempfile::tempdir().unwrap();
        write_minimal_config(td.path(), Some("http://127.0.0.1:8545")).unwrap();
        let body = std::fs::read_to_string(td.path().join("wavs.toml")).unwrap();
        assert!(body.contains("http://127.0.0.1:8545"));
        assert!(body.contains("[default]"));
        assert!(body.contains("[wavs]"));
    }

    #[test]
    fn write_minimal_config_omits_rpc_when_unset() {
        let td = tempfile::tempdir().unwrap();
        write_minimal_config(td.path(), None).unwrap();
        let body = std::fs::read_to_string(td.path().join("wavs.toml")).unwrap();
        assert!(!body.contains("[wavs.chains."));
    }
}
