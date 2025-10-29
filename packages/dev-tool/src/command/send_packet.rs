use alloy_signer_local::PrivateKeySigner;
use wavs_types::{
    aggregator::AddPacketRequest, Envelope, EnvelopeSigner, Packet, SignatureKind, TriggerData,
    WorkflowId,
};

use crate::service::create_service;

pub async fn run() {
    let service = create_service(None);
    let workflow_id = WorkflowId::new("dev-trigger-workflow".to_string()).unwrap();

    let signer = PrivateKeySigner::random();

    let envelope = Envelope {
        eventId: [1u8; 20].into(),
        ordering: [0u8; 12].into(),
        payload: vec![1, 2, 3, 4, 5].into(),
    };

    let signature = envelope
        .sign(&signer, SignatureKind::evm_default())
        .await
        .unwrap();

    let packet = Packet {
        envelope,
        workflow_id: workflow_id.clone(),
        service,
        signature,
        trigger_data: TriggerData::Raw(vec![1, 2, 3, 4, 5]),
    };

    let resp = reqwest::Client::new()
        .post("http://127.0.0.1:8001/packets")
        .header("Content-Type", "application/json")
        .json(&AddPacketRequest { packet })
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {}
        Ok(r) => panic!("Request failed: {}", r.status()),
        Err(e) => panic!("Request error: {e}"),
    }
}
