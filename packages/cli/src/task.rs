use std::time::Duration;

use utils::{
    avs_client::SignedData,
    eth_client::EthSigningClient,
    example_client::{SimpleSubmitClient, SimpleTriggerClient},
};

use crate::deploy::EthService;

pub async fn add_task(
    eth_signing_client: EthSigningClient,
    eth_service: &EthService,
    data: Vec<u8>,
) -> SignedData {
    // TODO - handle different kinds of triggers/submits
    let trigger_client =
        SimpleTriggerClient::new(eth_signing_client.clone(), eth_service.trigger_address);

    let submit_client = SimpleSubmitClient::new(
        eth_signing_client,
        eth_service.avs_addresses.service_manager,
    );

    let trigger_id = trigger_client.add_trigger(data).await.unwrap();

    tracing::info!("Task submitted with id: {}", trigger_id);

    tracing::info!("Waiting for the chain to see the result");

    tokio::time::timeout(Duration::from_secs(10), async move {
        loop {
            match submit_client.trigger_validated(trigger_id).await {
                true => {
                    let data = submit_client.trigger_data(trigger_id).await.unwrap();

                    let signature = submit_client.trigger_signature(trigger_id).await.unwrap();

                    return SignedData { data, signature };
                }
                false => {
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
