pub mod cosmos_stream;
pub mod cron_stream;
pub mod evm_stream;
pub mod local_command_stream;
pub mod svm_stream;

use crate::subsystems::trigger::streams::cosmos_stream::StreamTriggerCosmosContractEvent;
use crate::subsystems::trigger::streams::svm_stream::SvmProgramLog;

use super::{error::TriggerError, lookup::LookupId};
use futures::{stream::SelectAll, Stream};
use local_command_stream::LocalStreamCommand;
use std::pin::Pin;
use wavs_types::{SvmParsedEvent, ChainName, Timestamp};

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
        contract_events: Vec<StreamTriggerCosmosContractEvent>,
        block_height: u64,
    },
    Evm {
        chain_name: ChainName,
        log: Box<alloy_rpc_types_eth::Log>,
        block_number: u64,
        tx_hash: alloy_primitives::TxHash,
        log_index: u64,
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
    Svm {
        chain_name: ChainName,
        signature: String,
        slot: u64,
        program_logs: Vec<SvmProgramLog>,
        parsed_events: Vec<SvmParsedEvent>,
    },
    LocalCommand(LocalStreamCommand),
}
