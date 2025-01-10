use std::path::PathBuf;

use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
pub struct SquareRequest {
    pub x: u64,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct SquareResponse {
    pub y: u64,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryRequest {
    BlockHeight,
    Balance { address: Address },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryResponse {
    BlockHeight(u64),
    Balance(String),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PermissionsExampleRequest {
    pub url: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PermissionsExampleResponse {
    pub filename: PathBuf,
    pub contents: String,
    pub filecount: usize,
}
