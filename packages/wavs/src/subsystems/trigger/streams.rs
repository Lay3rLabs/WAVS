pub mod cosmos_stream;
pub mod cron_stream;
pub mod evm_stream;
pub mod local_command_stream;

use crate::subsystems::trigger::{
    streams::cosmos_stream::StreamTriggerCosmosContractEvent, TriggerCommand,
};

use super::{error::TriggerError, lookup::LookupId};
use futures::{stream::SelectAll, Stream};
use std::pin::Pin;
use wavs_types::{ChainKey, Timestamp};

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
        /// Unix timestamp (in nanos) when these triggers were processed
        trigger_time: Timestamp,
        /// Vector of lookup IDs for all triggers that are due at this time.
        /// Multiple triggers can fire simultaneously in a single tick.
        lookup_ids: Vec<LookupId>,
    },
    LocalCommand(TriggerCommand),
}
