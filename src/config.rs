use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub bind: Option<SocketAddr>,
    pub dir: Option<PathBuf>,
    pub envs: Option<Vec<[String; 2]>>,
}
