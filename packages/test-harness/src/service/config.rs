//! [`ServiceSpec`] — declarative description of the WAVS service a test wants to boot.
//!
//! `ServiceSpec` is a builder for the inputs the runner needs: which component WASM
//! drives the operator, which aggregator WASM drives the aggregator, what config
//! variables the component reads from `host::config_var()`, and how many operators
//! to register.
//!
//! The runner (in-process or subprocess) consumes a fully populated `ServiceSpec`
//! and is responsible for everything from registering operators through producing
//! signed submissions.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use wavs_types::{ChainConfigs, EvmChainConfig};

/// Declarative description of a service-under-test.
#[derive(Debug, Clone, Default)]
pub struct ServiceSpec {
    component_wasm: Option<PathBuf>,
    aggregator_wasm: Option<PathBuf>,
    config_vars: BTreeMap<String, String>,
    operator_count: Option<usize>,
    chain_configs: ChainConfigs,
}

impl ServiceSpec {
    /// Empty spec — fields are filled in via the builder methods.
    pub fn new() -> Self {
        Self::default()
    }

    /// Path to the operator component WASM (e.g. the compiled `delta_neutral_strategy`).
    pub fn component_wasm(mut self, path: impl AsRef<Path>) -> Self {
        self.component_wasm = Some(path.as_ref().to_path_buf());
        self
    }

    /// Path to the aggregator component WASM.
    pub fn aggregator_wasm(mut self, path: impl AsRef<Path>) -> Self {
        self.aggregator_wasm = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set a config var the component reads via `host::config_var()`.
    pub fn config_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config_vars.insert(key.into(), value.into());
        self
    }

    /// Set multiple config vars at once.
    pub fn config_vars<I, K, V>(mut self, pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (k, v) in pairs {
            self.config_vars.insert(k.into(), v.into());
        }
        self
    }

    /// Number of operators to register. Default 1.
    pub fn operator_count(mut self, n: usize) -> Self {
        self.operator_count = Some(n);
        self
    }

    /// Override the chain config registry passed to the wasmtime host.
    ///
    /// The aggregator world's `get_evm_chain_config(chain)` host call returns
    /// `None` against the default empty `ChainConfigs`, which causes
    /// `simple_aggregator.wasm` to fail with `no chain config for X`. Tests
    /// that drive the aggregator stage need to register at least the
    /// destination chain — see [`Self::with_evm_local_chain`] for the common
    /// local-Anvil case.
    pub fn chain_configs(mut self, configs: ChainConfigs) -> Self {
        self.chain_configs = configs;
        self
    }

    /// Register a local EVM chain (typically Anvil) by chain-key id with the
    /// spec's [`ChainConfigs`]. The resulting chain is reachable to the
    /// aggregator as `evm:<id>` (e.g. `with_evm_local_chain("local", endpoint)`
    /// registers `evm:local`).
    ///
    /// Convenience around `chain_configs.add_chain(...)` so consumers don't
    /// have to wire up the `EvmChainConfig` struct by hand.
    pub fn with_evm_local_chain(
        mut self,
        id: impl Into<String>,
        http_endpoint: impl Into<String>,
    ) -> Self {
        let id_str = id.into();
        let chain_key = format!("evm:{id_str}");
        self.chain_configs
            .add_chain(
                chain_key.parse().expect("evm:<id> must parse as ChainKey"),
                EvmChainConfig {
                    chain_id: id_str.parse().expect("chain id must parse"),
                    http_endpoint: Some(http_endpoint.into()),
                    ws_endpoints: vec![],
                    faucet_endpoint: None,
                    ws_priority_endpoint_index: None,
                }
                .into(),
            )
            .expect("add_chain should not fail for evm namespace");
        self
    }

    /// Validate the spec is complete enough to boot a service.
    pub fn validate(&self) -> Result<()> {
        let cw = self
            .component_wasm
            .as_ref()
            .ok_or_else(|| anyhow!("ServiceSpec missing component_wasm"))?;
        if !cw.exists() {
            return Err(anyhow!("component_wasm not found at {}", cw.display()));
        }
        let aw = self
            .aggregator_wasm
            .as_ref()
            .ok_or_else(|| anyhow!("ServiceSpec missing aggregator_wasm"))?;
        if !aw.exists() {
            return Err(anyhow!("aggregator_wasm not found at {}", aw.display()));
        }
        if let Some(0) = self.operator_count {
            return Err(anyhow!("operator_count cannot be 0"));
        }
        Ok(())
    }

    pub fn component_wasm_path(&self) -> Option<&Path> {
        self.component_wasm.as_deref()
    }

    pub fn aggregator_wasm_path(&self) -> Option<&Path> {
        self.aggregator_wasm.as_deref()
    }

    pub fn config_var_map(&self) -> &BTreeMap<String, String> {
        &self.config_vars
    }

    pub fn operators(&self) -> usize {
        self.operator_count.unwrap_or(1)
    }

    pub fn chain_configs_ref(&self) -> &ChainConfigs {
        &self.chain_configs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_requires_component_wasm() {
        let s = ServiceSpec::new();
        let e = s.validate().unwrap_err();
        assert!(format!("{e}").contains("component_wasm"));
    }

    #[test]
    fn validate_checks_paths_exist() {
        let s = ServiceSpec::new()
            .component_wasm("/no/such/component.wasm")
            .aggregator_wasm("/no/such/aggregator.wasm");
        let e = s.validate().unwrap_err();
        assert!(format!("{e}").contains("not found"));
    }

    #[test]
    fn config_vars_round_trip() {
        let s = ServiceSpec::new()
            .config_var("FOO", "1")
            .config_var("BAR", "2");
        let m = s.config_var_map();
        assert_eq!(m.get("FOO").map(String::as_str), Some("1"));
        assert_eq!(m.get("BAR").map(String::as_str), Some("2"));
    }

    #[test]
    fn operator_count_defaults_to_one() {
        assert_eq!(ServiceSpec::new().operators(), 1);
        assert_eq!(ServiceSpec::new().operator_count(3).operators(), 3);
    }
}
