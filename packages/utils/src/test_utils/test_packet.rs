use alloy_primitives::{Bytes, FixedBytes};
use alloy_signer::{k256::ecdsa::SigningKey, SignerSync};
use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
use alloy_sol_types::SolValue;
use wavs_types::{Envelope, EnvelopeExt, EnvelopeSignature, Packet, PacketRoute, ServiceID};

use super::test_contracts::ISimpleSubmit::DataWithId;

pub fn mock_packet(
    signer: &LocalSigner<SigningKey>,
    envelope: &Envelope,
    service_id: ServiceID,
) -> Packet {
    let signature = signer.sign_hash_sync(&envelope.eip191_hash()).unwrap();

    Packet {
        envelope: envelope.clone(),
        route: PacketRoute {
            service_id,
            workflow_id: "workflow".parse().unwrap(),
        },
        signature: EnvelopeSignature::Secp256k1(signature),
    }
}

pub fn mock_signer() -> LocalSigner<SigningKey> {
    MnemonicBuilder::<English>::default()
        .word_count(24)
        .build_random()
        .unwrap()
}

pub fn mock_envelope(trigger_id: u64, data: impl Into<Bytes>) -> Envelope {
    // SimpleSubmit has its own data format, so we need to encode it
    let payload = DataWithId {
        triggerId: trigger_id,
        data: data.into(),
    };
    Envelope {
        payload: payload.abi_encode().into(),
        eventId: FixedBytes([0; 20]),
        ordering: FixedBytes([0; 12]),
    }
}
