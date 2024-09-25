use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf};
use wasm_pkg_client::Registry;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub bind: Option<SocketAddr>,
    pub dir: Option<PathBuf>,
    pub envs: Option<Vec<[String; 2]>>,
    pub registry: Option<Registry>,
}
