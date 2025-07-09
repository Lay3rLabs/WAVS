use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Serialize, Deserialize, Debug)]
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
