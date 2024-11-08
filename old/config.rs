use cw_orch::environment::ChainKind;
use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct WasmaticConfig {
    pub bind: Option<SocketAddr>,
    pub dir: Option<PathBuf>,
    pub envs: Option<Vec<[String; 2]>>,
    pub chain_kind: Option<ChainKind>,
    pub grpc_url: Option<String>,
    pub chain_id: Option<String>,
    pub gas_denom: Option<String>,
    pub gas_price: Option<f64>,
    pub cors_allowed_origins: Option<Vec<String>>,
}
