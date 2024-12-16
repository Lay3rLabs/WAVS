use std::time::Duration;

use utils::{eth_client::EthSigningClient, layer_contract_client::LayerContractClientSimple};
use wavs::apis::{ServiceID, WorkflowID};

pub async fn run_eth_trigger_echo_task(
    eth_signing_client: EthSigningClient,
    wavs: bool,
    service_id: ServiceID,
    workflow_id: WorkflowID,
    trigger_address: alloy::primitives::Address,
    service_manager_address: alloy::primitives::Address,
    name: String,
) -> String {
    let client = LayerContractClientSimple::new(
        eth_signing_client,
        service_manager_address,
        trigger_address,
    );

    let data = name.as_bytes().to_vec();

    let trigger_id = client
        .trigger
        .add_trigger(
            service_id.to_string(),
            workflow_id.to_string(),
            data.clone(),
        )
        .await
        .unwrap();

    println!("Task submitted with id: {}", trigger_id);

    if !wavs {
        tracing::info!("Submitting the task result directly");

        client
            .add_signed_trigger_data(trigger_id, data)
            .await
            .unwrap();
    }

    tracing::info!("Waiting for the chain to see the result");

    tokio::time::timeout(Duration::from_secs(10), async move {
        loop {
            let signature = client.get_signed_data(trigger_id).await.unwrap().signature;

            if !signature.is_empty() {
                return hex::encode(signature);
            } else {
                tracing::info!("Waiting for task response on {}", trigger_id);
            }
            // still open, waiting...
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap()
}
