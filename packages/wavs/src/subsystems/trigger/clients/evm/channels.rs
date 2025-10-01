use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::subsystems::trigger::clients::evm::{connection::ConnectionState, rpc::RpcRequest};

use super::connection::ConnectionData;

pub struct Channels {
    pub connection: ConnectionChannels,
    pub subscription: SubscriptionChannels,
    pub client: ClientChannels,
}

pub struct ConnectionChannels {
    pub connection_send_rx: UnboundedReceiver<RpcRequest>,
    pub connection_data_tx: UnboundedSender<ConnectionData>,
    pub connection_state_tx: UnboundedSender<ConnectionState>,
}

pub struct SubscriptionChannels {
    pub subscription_block_height_tx: UnboundedSender<u64>,
    pub connection_send_tx: UnboundedSender<RpcRequest>,
    pub connection_state_rx: UnboundedReceiver<ConnectionState>,
    pub connection_data_rx: UnboundedReceiver<ConnectionData>,
}

pub struct ClientChannels {
    pub subscription_block_height_rx: UnboundedReceiver<u64>,
}

impl Channels {
    pub fn new() -> Self {
        let (connection_data_tx, connection_data_rx) = tokio::sync::mpsc::unbounded_channel();
        let (connection_send_tx, connection_send_rx) = tokio::sync::mpsc::unbounded_channel();
        let (connection_state_tx, connection_state_rx) = tokio::sync::mpsc::unbounded_channel();
        let (subscription_block_height_tx, subscription_block_height_rx) =
            tokio::sync::mpsc::unbounded_channel();

        Self {
            connection: ConnectionChannels {
                connection_send_rx,
                connection_data_tx,
                connection_state_tx,
            },
            subscription: SubscriptionChannels {
                subscription_block_height_tx,
                connection_send_tx,
                connection_state_rx,
                connection_data_rx,
            },
            client: ClientChannels {
                subscription_block_height_rx,
            },
        }
    }
}
