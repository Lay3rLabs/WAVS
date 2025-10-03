use alloy_primitives::B256;
use alloy_rpc_types_eth::Log;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::subsystems::trigger::clients::evm::{
    connection::ConnectionState, rpc_types::outbound::RpcRequest,
};

use super::connection::ConnectionData;

pub struct Channels {
    pub connection: ConnectionChannels,
    pub subscription: SubscriptionChannels,
    pub client: ClientChannels,
}

pub struct ConnectionChannels {
    pub connection_send_rpc_rx: UnboundedReceiver<RpcRequest>,
    pub connection_data_tx: UnboundedSender<ConnectionData>,
    pub connection_state_tx: UnboundedSender<ConnectionState>,
}

pub struct SubscriptionChannels {
    pub subscription_block_height_tx: UnboundedSender<u64>,
    pub subscription_log_tx: UnboundedSender<Log>,
    pub subscription_new_pending_transaction_tx: UnboundedSender<B256>,
    pub connection_send_rpc_tx: UnboundedSender<RpcRequest>,
    pub connection_state_rx: UnboundedReceiver<ConnectionState>,
    pub connection_data_rx: UnboundedReceiver<ConnectionData>,
}

pub struct ClientChannels {
    pub subscription_block_height_rx: UnboundedReceiver<u64>,
    pub subscription_log_rx: UnboundedReceiver<Log>,
    pub subscription_new_pending_transaction_rx: UnboundedReceiver<B256>,
}

impl Channels {
    pub fn new() -> Self {
        let (connection_data_tx, connection_data_rx) = tokio::sync::mpsc::unbounded_channel();
        let (connection_send_rpc_tx, connection_send_rpc_rx) =
            tokio::sync::mpsc::unbounded_channel();
        let (connection_state_tx, connection_state_rx) = tokio::sync::mpsc::unbounded_channel();
        let (subscription_block_height_tx, subscription_block_height_rx) =
            tokio::sync::mpsc::unbounded_channel();
        let (subscription_log_tx, subscription_log_rx) = tokio::sync::mpsc::unbounded_channel();
        let (subscription_new_pending_transaction_tx, subscription_new_pending_transaction_rx) =
            tokio::sync::mpsc::unbounded_channel();

        Self {
            connection: ConnectionChannels {
                connection_send_rpc_rx,
                connection_data_tx,
                connection_state_tx,
            },
            subscription: SubscriptionChannels {
                subscription_block_height_tx,
                subscription_log_tx,
                subscription_new_pending_transaction_tx,
                connection_send_rpc_tx,
                connection_state_rx,
                connection_data_rx,
            },
            client: ClientChannels {
                subscription_block_height_rx,
                subscription_log_rx,
                subscription_new_pending_transaction_rx,
            },
        }
    }
}
