use crate::service::SERVICE;

pub async fn run(count: usize) {
    let service_id = SERVICE.id();
    let workflow_id = SERVICE.workflows.keys().next().unwrap().clone();

    let body = wavs_types::SimulatedTriggerRequest {
        service_id,
        workflow_id,
        trigger: wavs_types::Trigger::Manual,
        data: wavs_types::TriggerData::Raw("hello world!".as_bytes().to_vec()),
        count,
    };

    let resp = reqwest::Client::new()
        .post("http://localhost:8000/dev/triggers".to_string())
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {}
        Ok(r) => panic!("Request failed: {}", r.status()),
        Err(e) => panic!("Request error: {e}"),
    }
}
