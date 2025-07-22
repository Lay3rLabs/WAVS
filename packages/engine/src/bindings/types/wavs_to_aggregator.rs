use crate::bindings::aggregator::world::wavs::{
    aggregator::aggregator::{
        Envelope as WitEnvelope, EnvelopeSignature as WitEnvelopeSignature, Packet as WitPacket,
        Secp256k1Signature as WitSecp256k1Signature,
    },
    types::{
        chain::EvmAddress as WitEvmAddress,
        service::{
            EvmManager as WitEvmManager, Service as WitService,
            ServiceManager as WitServiceManager, ServiceStatus as WitServiceStatus,
        },
    },
};
use wavs_types::{Envelope, EnvelopeSignature, Packet};

impl From<Packet> for WitPacket {
    fn from(packet: Packet) -> Self {
        WitPacket {
            service: packet.service.into(),
            workflow_id: packet.workflow_id.to_string(),
            envelope: packet.envelope.into(),
            signature: packet.signature.into(),
        }
    }
}

impl From<wavs_types::Service> for WitService {
    fn from(service: wavs_types::Service) -> Self {
        WitService {
            name: service.name,
            workflows: vec![], // Simplified for now
            status: service.status.into(),
            manager: service.manager.into(),
        }
    }
}

impl From<wavs_types::ServiceStatus> for WitServiceStatus {
    fn from(status: wavs_types::ServiceStatus) -> Self {
        match status {
            wavs_types::ServiceStatus::Active => WitServiceStatus::Active,
            wavs_types::ServiceStatus::Paused => WitServiceStatus::Paused,
        }
    }
}

impl From<wavs_types::ServiceManager> for WitServiceManager {
    fn from(manager: wavs_types::ServiceManager) -> Self {
        match manager {
            wavs_types::ServiceManager::Evm {
                chain_name,
                address,
            } => WitServiceManager::Evm(WitEvmManager {
                chain_name: chain_name.to_string(),
                address: WitEvmAddress {
                    raw_bytes: address.to_vec(),
                },
            }),
        }
    }
}

impl From<Envelope> for WitEnvelope {
    fn from(envelope: Envelope) -> Self {
        WitEnvelope {
            event_id: envelope.eventId.to_vec(),
            ordering: envelope.ordering.to_vec(),
            payload: envelope.payload.to_vec(),
        }
    }
}

impl From<EnvelopeSignature> for WitEnvelopeSignature {
    fn from(signature: EnvelopeSignature) -> Self {
        match signature {
            EnvelopeSignature::Secp256k1(sig) => {
                WitEnvelopeSignature::Secp256k1(WitSecp256k1Signature {
                    signature_data: sig.as_bytes().to_vec(),
                })
            }
        }
    }
}
