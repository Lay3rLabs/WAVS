pub mod atproto_jetstream;
pub mod cosmos_stream;
pub mod cron_stream;
pub mod evm_stream;
pub mod local_command_stream;

use crate::subsystems::trigger::{
    streams::atproto_jetstream::AtProtoEvent,
    streams::cosmos_stream::StreamTriggerCosmosContractEvent, TriggerCommand,
};

use super::{error::TriggerError, lookup::LookupId};
use futures::{stream::SelectAll, Stream};
use std::pin::Pin;
use wavs_types::{ChainKey, Timestamp};

/// Represents a cron trigger firing event at a specific scheduled time.
/// Multiple lookup IDs can fire at the same timestamp.
#[derive(Debug, Clone)]
pub struct CronHit {
    /// All lookup IDs that are scheduled to fire at this timestamp
    pub lookup_ids: Vec<LookupId>,
    /// The scheduled time when these triggers should fire
    pub scheduled_time: Timestamp,
}

pub type MultiplexedStream = SelectAll<
    Pin<Box<dyn Stream<Item = std::result::Result<StreamTriggers, TriggerError>> + Send>>,
>;

// *potential* triggers that we can react to
// this is just a local encapsulation, not a full trigger
// and is used to ultimately filter+map to a TriggerAction
#[derive(Debug)]
pub enum StreamTriggers {
    Cosmos {
        chain: ChainKey,
        // these are not filtered yet, just all the contract-based events
        contract_events: Vec<StreamTriggerCosmosContractEvent>,
        block_height: u64,
    },
    Evm {
        chain: ChainKey,
        log: Box<alloy_rpc_types_eth::Log>,
        block_number: u64,
        tx_hash: alloy_primitives::TxHash,
        block_hash: alloy_primitives::BlockHash,
        tx_index: u64,
        block_timestamp: Option<u64>,
        log_index: u64,
    },
    // We need a separate stream for EVM block interval triggers
    EvmBlock {
        chain: ChainKey,
        block_height: u64,
    },
    Cron {
        /// Vector of cron hits for this tick.
        /// Each hit represents a specific scheduled time with all lookup IDs
        /// that should fire at that time.
        hits: Vec<CronHit>,
    },
    LocalCommand(TriggerCommand),
    AtProto {
        event: AtProtoEvent,
    },
}
