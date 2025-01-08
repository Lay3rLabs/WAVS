use std::time::Duration;

use utils::{
    eth_client::EthSigningClient,
    layer_contract_client::{LayerAddresses, LayerContractClientSimple, SignedData},
};

pub async fn add_task(
    eth_signing_client: EthSigningClient,
    service_addresses: &LayerAddresses,
    data: Vec<u8>,
) -> SignedData {
    let client = LayerContractClientSimple::new(
        eth_signing_client,
        service_addresses.service_manager,
        service_addresses.trigger,
    );

    let trigger_id = client.trigger.add_trigger(data).await.unwrap();

    tracing::info!("Task submitted with id: {}", trigger_id);

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
