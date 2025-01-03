use std::time::Duration;

use utils::{
    eth_client::EthSigningClient,
    layer_contract_client::{LayerAddresses, LayerContractClientSimple, SignedData},
};
use wavs::apis::{ServiceID, WorkflowID};

pub async fn add_task(
    eth_signing_client: EthSigningClient,
    wavs: bool,
    service_id: ServiceID,
    workflow_id: WorkflowID,
    service_addresses: &LayerAddresses,
    data: Vec<u8>,
) -> SignedData {
    let client = LayerContractClientSimple::new(
        eth_signing_client,
        service_addresses.service_manager,
        service_addresses.trigger,
    );

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
            .add_signed_payload(client.sign_payload(trigger_id, data).await.unwrap())
            .await
            .unwrap();
    }

    tracing::info!("Waiting for the chain to see the result");

    tokio::time::timeout(Duration::from_secs(10), async move {
        loop {
            let resp = client.load_signed_data(trigger_id).await.unwrap();

            match resp {
                Some(resp) => {
                    return resp;
                }
                None => {
                    tracing::info!("Waiting for task response on {}", trigger_id);
                }
            }
            // still open, waiting...
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap()
}
