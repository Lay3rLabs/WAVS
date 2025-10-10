use std::{collections::HashSet, sync::Arc};

use alloy_primitives::{Address, B256};
use slotmap::{new_key_type, SlotMap};

#[derive(Clone)]
pub struct RpcIds {
    lookup: Arc<std::sync::RwLock<SlotMap<RpcId, RpcRequestKind>>>,
}

impl RpcIds {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            lookup: Arc::new(std::sync::RwLock::new(SlotMap::with_key())),
        }
    }

    pub fn insert(&self, kind: RpcRequestKind) -> RpcId {
        self.lookup.write().unwrap().insert(kind)
    }

    pub fn kind(&self, id: RpcId) -> Option<RpcRequestKind> {
        self.lookup.read().unwrap().get(id).cloned()
    }

    pub fn clear_all(&self) {
        self.lookup.write().unwrap().clear();
    }
}

new_key_type! {
    pub struct RpcId;
}

#[derive(Debug, Clone, PartialEq)]
pub enum RpcRequestKind {
    SubscribeNewHeads,
    SubscribeLogs {
        addresses: HashSet<Address>,
        topics: HashSet<B256>,
    },
    SubscribeNewPendingTransactions,
    Unsubscribe {
        subscription_id: String,
    },
}
