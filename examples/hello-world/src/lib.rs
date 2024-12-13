#[allow(warnings)]
mod bindings;

use alloy_primitives::{eip191_hash_message, keccak256, Address, FixedBytes, Log};
use alloy_sol_macro::sol;
use alloy_sol_types::{SolEvent, SolValue};
use bindings::{EthLog, Guest, Response};

struct Component;

sol! {
    event NewTaskCreated(uint32 indexed taskIndex, Task task);

    struct Task {
        string name;
        uint32 taskCreatedBlock;
    }
}

impl Guest for Component {
    fn process_eth_event(input: EthLog) -> Result<Response, String> {
        let EthLog {
            address,
            log_topics,
            log_data,
        } = input;

        let address = Address::from_slice(&address);
        let log_topics = log_topics
            .into_iter()
            .map(|t| FixedBytes::<32>::from_slice(t.as_slice()))
            .collect();
        let log = Log::new(address, log_topics, log_data.into())
            .ok_or("Failed to create log from event output")?;

        let event = NewTaskCreated::decode_log(&log, false)
            .map_err(|e| format!("Failed to decode log into NewTaskCreated event: {:?}", e))?;

        let message = format!("Hello, {}", event.task.name);
        let message_hash = eip191_hash_message(keccak256(message.abi_encode_packed()));

        Ok(Response {
            message_hash: message_hash.to_vec(),
            task_name: event.task.name.to_string(),
            task_created_block: event.task.taskCreatedBlock,
            task_index: event.taskIndex,
        })
    }
}

bindings::export!(Component with_types_in bindings);
