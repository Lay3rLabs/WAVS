use std::{collections::HashMap, path::PathBuf};

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
    BlockHeight { chain: String },
    Balance { chain: String, address: Address },
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
        bucket: String,
        key: String,
        value: Vec<u8>,
    },
    Read {
        bucket: String,
        key: String,
    },
    AtomicIncrement {
        bucket: String,
        key: String,
        delta: i64,
    },
    AtomicSwap {
        bucket: String,
        key: String,
        value: Vec<u8>,
    },
    AtomicRead {
        bucket: String,
        key: String,
    },
    BatchRead {
        bucket: String,
        keys: Vec<String>,
    },
    BatchWrite {
        bucket: String,
        values: HashMap<String, Vec<u8>>,
    },
    BatchDelete {
        bucket: String,
        keys: Vec<String>,
    },
    ListKeys {
        bucket: String,
        cursor: Option<String>,
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
    Read {
        value: Vec<u8>,
    },
    // returns the new value after increment
    AtomicIncrement {
        value: i64,
    },
    AtomicSwap,
    AtomicRead {
        value: Vec<u8>,
    },
    BatchRead {
        values: HashMap<String, Vec<u8>>,
    },
    BatchWrite,
    BatchDelete,
    ListKeys {
        keys: Vec<String>,
        cursor: Option<String>,
    },
}

#[derive(Error, Debug)]
pub enum KvStoreError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to open bucket {id}: {reason}")]
    BucketOpen { id: String, reason: String },
    #[error("Failed to read key {key} for bucket {bucket}: {reason}")]
    ReadKey {
        bucket: String,
        key: String,
        reason: String,
    },
    #[error("Failed to writekey {key} for bucket {bucket}: {reason}")]
    WriteKey {
        bucket: String,
        key: String,
        reason: String,
    },
    #[error("Missing key: {key} for bucket {bucket}")]
    MissingKey { bucket: String, key: String },
    #[error("Failed to atomically increment bucket {bucket}, key {key}, delta {delta}: {reason}")]
    AtomicIncrement {
        bucket: String,
        key: String,
        delta: i64,
        reason: String,
    },
    #[error("Failed to atomically swap bucket {bucket}, key {key}: {reason}")]
    AtomicSwap {
        bucket: String,
        key: String,
        reason: String,
    },
    #[error("Failed to acquire atomic CAS lock for bucket {bucket}, key {key}: {reason}")]
    AtomicCasResource {
        bucket: String,
        key: String,
        reason: String,
    },
    #[error("Failed to read atomic value for bucket {bucket}, key {key}: {reason}")]
    AtomicRead {
        bucket: String,
        key: String,
        reason: String,
    },
    #[error("Failed to perform batch operation for bucket {bucket}, {reason}")]
    BatchRead { bucket: String, reason: String },
    #[error("Failed to perform batch write for bucket {bucket}, {reason}")]
    BatchWrite { bucket: String, reason: String },
    #[error("Failed to perform batch delete for bucket {bucket}, {reason}")]
    BatchDelete { bucket: String, reason: String },
    #[error("Failed to list keys for bucket {bucket}, cursor: {cursor:?}: {reason}")]
    ListKeys {
        bucket: String,
        cursor: Option<String>,
        reason: String,
    },
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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct BlockIntervalResponse {
    pub trigger_config_start: Option<u64>,
    pub trigger_config_end: Option<u64>,
    pub trigger_config_interval: u32,
    pub trigger_data_block_height: u64,
    pub count: u64,
}
