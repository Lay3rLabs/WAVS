use wavs_types::{
    aggregator::AddPacketRequest, Envelope, EnvelopeSignature, Packet, SignatureKind, TriggerData,
    WorkflowId,
};

use crate::service::SERVICE;

pub async fn run() {
    let service = SERVICE.clone();
    let workflow_id = WorkflowId::new("dev-trigger-workflow".to_string()).unwrap();

    let packet = Packet {
        envelope: Envelope {
            eventId: [0u8; 20].into(),
            ordering: [0u8; 12].into(),
            payload: vec![1, 2, 3, 4, 5].into(),
        },
        workflow_id: workflow_id.clone(),
        service,
        signature: EnvelopeSignature {
            data: alloy_primitives::Signature::from_bytes_and_parity(&[0u8; 64], false).into(),
            kind: SignatureKind::evm_default(),
        },
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
