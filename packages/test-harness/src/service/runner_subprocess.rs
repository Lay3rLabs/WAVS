//! Subprocess runner — spawns the real `wavs` binary as a child process.
//!
//! **Preview status.** The subprocess tier ships in this release with API shape
//! locked but lifecycle methods stubbed. The intent is that consumers can write
//! their test harness against this surface and the implementation can be filled
//! in without breaking their tests.
//!
//! What ships now:
//! - [`SubprocessConfig`] — full builder for the inputs the runner needs.
//! - [`SubprocessRunner::start`] — returns a descriptive `unimplemented` error.
//!
//! What's planned (tracked under Lay3rLabs/WAVS#1147 follow-ups):
//! - Spawn `wavs` with `--dev-endpoints-enabled=true
//!   --disable-trigger-networking=true --disable-submission-networking=true`,
//!   piping `WAVS_HOME` and `WAVS_DATA` to a tempdir.
//! - HTTP health-probe loop until the node accepts service registrations.
//! - Service deployment via the HTTP `/services` endpoint.
//! - Trigger emission via dev-tool's `send-triggers` path.
//! - Quorum + submission waiting via HTTP polling.
//! - Clean shutdown via SIGINT + tempdir cleanup on Drop.
//!
//! Tests that need real end-to-end coverage should use [`super::InProcRunner`]
//! for now. Tests can be authored against this API; today they'll surface the
//! preview error message.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::service::ServiceSpec;

/// Configuration for the subprocess WAVS runner.
#[derive(Debug, Clone, Default)]
pub struct SubprocessConfig {
    wavs_binary: Option<PathBuf>,
    rpc_url: Option<String>,
    data_dir: Option<PathBuf>,
    extra_env: Vec<(String, String)>,
}

impl SubprocessConfig {
    /// Empty config — fill via the builder methods.
    pub fn new() -> Self {
        Self::default()
    }

    /// Path to the `wavs` binary. If unset, the runner attempts to locate it via
    /// `$PATH` or by building it from the WAVS repo (preview behavior).
    pub fn wavs_binary(mut self, path: impl AsRef<Path>) -> Self {
        self.wavs_binary = Some(path.as_ref().to_path_buf());
        self
    }

    /// EVM RPC URL the WAVS node connects to (Anvil endpoint or fork URL).
    pub fn rpc_url(mut self, url: impl Into<String>) -> Self {
        self.rpc_url = Some(url.into());
        self
    }

    /// Directory the WAVS node uses for `WAVS_DATA`. Defaults to a tempdir.
    pub fn data_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.data_dir = Some(path.as_ref().to_path_buf());
        self
    }

    /// Add a `(KEY, VALUE)` pair to the spawned process environment.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_env.push((key.into(), value.into()));
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

/// Subprocess runner handle.
///
/// **Preview**: `start` returns an explicit `unimplemented` error today.
pub struct SubprocessRunner {
    #[allow(dead_code)]
    config: SubprocessConfig,
    #[allow(dead_code)]
    spec: ServiceSpec,
}

impl SubprocessRunner {
    /// Build a subprocess runner from a config and service spec.
    pub fn new(config: SubprocessConfig, spec: ServiceSpec) -> Result<Self> {
        spec.validate()?;
        Ok(Self { config, spec })
    }

    /// Start the WAVS subprocess.
    ///
    /// **Preview**: Not yet implemented. Returns an error directing callers to
    /// either use [`super::InProcRunner`] or track the follow-up issue.
    pub async fn start(&self) -> Result<()> {
        Err(anyhow!(
            "subprocess tier is preview — use InProcRunner for now; \
             see packages/test-harness/src/service/runner_subprocess.rs and \
             Lay3rLabs/WAVS#1147 follow-ups for the planned implementation"
        ))
    }
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
}
