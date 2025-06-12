pub mod cosmos_stream;
pub mod cron_stream;
pub mod evm_stream;
pub mod local_command_stream;

use super::{error::TriggerError, lookup::LookupId};
use futures::{stream::SelectAll, Stream};
use local_command_stream::LocalStreamCommand;
use std::pin::Pin;
use wavs_types::{ChainName, Timestamp};

pub type MultiplexedStream = SelectAll<
    Pin<Box<dyn Stream<Item = std::result::Result<StreamTriggers, TriggerError>> + Send>>,
>;

// *potential* triggers that we can react to
// this is just a local encapsulation, not a full trigger
// and is used to ultimately filter+map to a TriggerAction
#[derive(Debug)]
pub enum StreamTriggers {
    Cosmos {
        chain_name: ChainName,
        // these are not filtered yet, just all the contract-based events
        contract_events: Vec<(layer_climb::prelude::Address, cosmwasm_std::Event)>,
        block_height: u64,
    },
    Evm {
        chain_name: ChainName,
        log: alloy_rpc_types_eth::Log,
        block_height: u64,
    },
    // We need a separate stream for EVM block interval triggers
    EvmBlock {
        chain_name: ChainName,
        block_height: u64,
    },
    Cron {
        /// Unix timestamp (in nanos) when these triggers were processed
        trigger_time: Timestamp,
        /// Vector of lookup IDs for all triggers that are due at this time.
        /// Multiple triggers can fire simultaneously in a single tick.
        lookup_ids: Vec<LookupId>,
    },
    LocalCommand(LocalStreamCommand),
}
