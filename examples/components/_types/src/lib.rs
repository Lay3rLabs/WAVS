use std::path::PathBuf;

use layer_climb_address::Address;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ChainTriggerLookupResponse {
    pub data: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryRequest {
    BlockHeight {
        chain_name: String,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryResponse {
    BlockHeight(u64),
    Balance(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KvStoreRequest {
    Write {
        key: String,
        value: Vec<u8>,
        read_immediately: bool, // Optional flag to read immediately after writing
    },
    Read {
        key: String,
    },
}

impl KvStoreRequest {
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KvStoreResponse {
    Write,
    Read { value: Vec<u8> },
}

#[derive(Error, Debug)]
pub enum KvStoreError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),
}

pub type KvStoreResult<T> = Result<T, KvStoreError>;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PermissionsResponse {
    pub filename: PathBuf,
    pub contents: String,
    pub filecount: usize,
    // derived from host get-service call
    pub digest: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SquareRequest {
    pub x: u64,
}

impl SquareRequest {
    pub fn new(x: u64) -> Self {
        SquareRequest { x }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SquareResponse {
    pub y: u64,
}

impl SquareResponse {
    pub fn new(y: u64) -> Self {
        SquareResponse { y }
    }
}
