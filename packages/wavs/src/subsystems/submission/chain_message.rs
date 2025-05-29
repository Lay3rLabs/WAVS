use wavs_types::{Envelope, PacketRoute, Submit};

/// The data returned from a trigger action
#[derive(Clone, Debug)]
pub struct ChainMessage {
    pub packet_route: PacketRoute,
    pub envelope: Envelope,
    pub submit: Submit,
}
