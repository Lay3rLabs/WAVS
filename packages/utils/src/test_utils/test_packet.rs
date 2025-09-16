use alloy_primitives::{Bytes, FixedBytes};
use alloy_signer::{k256::ecdsa::SigningKey, SignerSync};
use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
use alloy_sol_types::SolValue;
use wavs_types::{
    Component, ComponentDigest, ComponentSource, Envelope, EnvelopeExt, EnvelopeSignature, Packet,
    Service, ServiceManager, ServiceStatus, SignatureKind, Submit, Trigger, TriggerData, Workflow,
    WorkflowId,
};

use crate::test_utils::address::rand_address_evm;

use super::test_contracts::ISimpleSubmit::DataWithId;

pub fn packet_from_service(
    signer: &LocalSigner<SigningKey>,
    service: &Service,
    workflow_id: &WorkflowId,
    envelope: &Envelope,
    trigger_data: &TriggerData,
) -> Packet {
    let signature = signer
        .sign_hash_sync(&envelope.prefix_eip191_hash())
        .unwrap();

    Packet {
        service: service.clone(),
        workflow_id: workflow_id.clone(),
        envelope: envelope.clone(),
        signature: EnvelopeSignature {
            data: signature.into(),
            kind: SignatureKind::evm_default(),
        },
        trigger_data: trigger_data.clone(),
    }
}
pub fn mock_packet(
    signer: &LocalSigner<SigningKey>,
    envelope: &Envelope,
    workflow_id: WorkflowId,
) -> Packet {
    let service = Service {
        name: "mock packet service".to_string(),
        workflows: [(
            workflow_id.clone(),
            Workflow {
                trigger: Trigger::Manual,
                component: Component::new(ComponentSource::Digest(ComponentDigest::hash([0; 32]))),
                submit: Submit::None,
            },
        )]
        .into(),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: "evm:local".parse().unwrap(),
            address: rand_address_evm(),
        },
    };

    packet_from_service(
        signer,
        &service,
        &workflow_id,
        envelope,
        &TriggerData::default(),
    )
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
