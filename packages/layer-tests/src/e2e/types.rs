use std::path::PathBuf;

use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use wavs_types::ChainName;

#[derive(Serialize, Deserialize, Debug)]
pub struct SquareRequest {
    pub x: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SquareResponse {
    pub y: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryRequest {
    BlockHeight {
        chain_name: ChainName,
    },
    Balance {
        chain_name: String,
        address: Address,
    },
}

impl CosmosQueryRequest {
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryResponse {
    BlockHeight(u64),
    Balance(String),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PermissionsRequest {
    pub get_url: String,
    pub post_url: String,
    pub post_data: (String, String),
    pub timestamp: u64,
}

impl PermissionsRequest {
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PermissionsResponse {
    pub filename: PathBuf,
    pub contents: String,
    pub filecount: usize,
}
