use alloy_provider::{DynProvider, Provider};
use alloy_rpc_types_eth::TransactionReceipt;
use layer_climb::prelude::CosmosAddr;
use serde::{Deserialize, Serialize};
use utils::evm_client::EvmSigningClient;
use wavs_types::{
    contracts::cosmwasm::{
        service_handler::{ServiceHandlerExecuteMessages, ServiceHandlerQueryMessages},
        service_manager::{
            error::WavsValidateError, ServiceManagerQueryMessages, WavsValidateResult,
        },
    },
    CosmosSubmitAction, EvmSubmitAction,
    IWavsServiceHandler::IWavsServiceHandlerInstance,
    IWavsServiceManager::IWavsServiceManagerInstance,
    ServiceManagerError, Submission, WavsSignature, WavsSigner,
};

use crate::subsystems::aggregator::{error::AggregatorError, Aggregator};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AnyTransactionReceipt {
    Evm(Box<TransactionReceipt>),
    // tx hash
    Cosmos(String),
}

impl AnyTransactionReceipt {
    pub fn tx_hash(&self) -> String {
        match self {
            AnyTransactionReceipt::Evm(receipt) => format!("{}", receipt.transaction_hash),
            AnyTransactionReceipt::Cosmos(tx_hash) => tx_hash.clone(),
        }
    }
}

impl Aggregator {
    pub async fn handle_action_submit_evm(
        &self,
        client: EvmSigningClient,
        queue: &[Submission],
        action: EvmSubmitAction,
    ) -> Result<AnyTransactionReceipt, AggregatorError> {
        tracing::info!("Handling submit for {}", queue.last().unwrap().label());
        let contract_address = action.address.into();

        let service_manager = self
            .evm_get_submission_service_manager(client.provider.clone(), contract_address)
            .await?;

        // TODO - query to see if we should submit at all (e.g. has it already been submitted?)

        let block_height_minus_one = service_manager
            .provider()
            .get_block_number()
            .await
            .map_err(|e| AggregatorError::BlockNumber(e.into()))?
            - 1;

        let signatures: Vec<WavsSignature> = queue
            .iter()
            .map(|queued| queued.envelope_signature.clone())
            .collect();

        // safe - we pushed the latest submission into the (temporary) queue
        let signature_data = queue
            .first()
            .unwrap()
            .envelope
            .signature_data(signatures, block_height_minus_one)?;

        let result = service_manager
            .validate(
                queue.first().unwrap().envelope.clone().into(),
                signature_data.clone().into(),
            )
            .call()
            .await;

        if let Err(err) = result {
            match err.as_decoded_interface_error::<ServiceManagerError>() {
                Some(err) => match err {
                    ServiceManagerError::InsufficientQuorum(info) => {
                        return Err(AggregatorError::InsufficientQuorum {
                            signer_weight: info.signerWeight.to_string(),
                            threshold_weight: info.thresholdWeight.to_string(),
                            total_weight: info.totalWeight.to_string(),
                        });
                    }
                    err => {
                        return Err(AggregatorError::EvmServiceManagerValidateKnown(err));
                    }
                },
                None => match err.as_revert_data() {
                    Some(raw) => {
                        return Err(AggregatorError::EvmServiceManagerValidateAnyRevert(
                            raw.to_string(),
                        ));
                    }
                    None => {
                        return Err(AggregatorError::EvmServiceManagerValidateUnknown(err));
                    }
                },
            }
        };

        let tx_receipt = client
            .send_envelope_signatures(
                queue.first().unwrap().envelope.clone(),
                signature_data,
                contract_address,
                None,
                action.gas_price,
            )
            .await?;

        Ok(AnyTransactionReceipt::Evm(Box::new(tx_receipt)))
    }

    pub async fn handle_action_submit_cosmos(
        &self,
        client: layer_climb::prelude::SigningClient,
        queue: &[Submission],
        action: CosmosSubmitAction,
    ) -> Result<AnyTransactionReceipt, AggregatorError> {
        let service_manager_addr: CosmosAddr = client
            .querier
            .contract_smart(
                &action.address.clone().into(),
                &ServiceHandlerQueryMessages::WavsServiceManager {},
            )
            .await
            .map_err(AggregatorError::CosmosClient)?;

        let block_height_minus_one = client
            .querier
            .block_height()
            .await
            .map_err(AggregatorError::BlockNumber)?
            - 1;

        let signatures: Vec<WavsSignature> = queue
            .iter()
            .map(|queued| queued.envelope_signature.clone())
            .collect();

        // safe - we pushed the latest submission into the (temporary) queue
        let signature_data = queue
            .first()
            .unwrap()
            .envelope
            .signature_data(signatures, block_height_minus_one)?;

        let result: WavsValidateResult = client
            .querier
            .contract_smart(
                &service_manager_addr.into(),
                &ServiceManagerQueryMessages::WavsValidate {
                    envelope: queue.first().unwrap().envelope.clone().into(),
                    signature_data: signature_data.clone().into(),
                },
            )
            .await
            .map_err(AggregatorError::CosmosClient)?;

        match result {
            WavsValidateResult::Ok => {
                tracing::info!("Service manager validation passed for custom submit");
            }
            WavsValidateResult::Err(err) => match err {
                WavsValidateError::InsufficientQuorum {
                    signer_weight,
                    threshold_weight,
                    total_weight,
                } => {
                    return Err(AggregatorError::InsufficientQuorum {
                        signer_weight: signer_weight.to_string(),
                        threshold_weight: threshold_weight.to_string(),
                        total_weight: total_weight.to_string(),
                    });
                }
                err => {
                    return Err(AggregatorError::CosmosServiceManagerValidate(err));
                }
            },
        }

        // if we reach here, validation passed and we should submit!

        let resp = client
            .contract_execute(
                &action.address.into(),
                &ServiceHandlerExecuteMessages::WavsHandleSignedEnvelope {
                    envelope: queue.first().unwrap().envelope.clone().into(),
                    signature_data: signature_data.clone().into(),
                },
                vec![],
                None,
            )
            .await
            .map_err(AggregatorError::CosmosClient)?;

        tracing::info!("Custom submit transaction sent: {:?}", resp.txhash);

        Ok(AnyTransactionReceipt::Cosmos(resp.txhash))
    }

    async fn evm_get_submission_service_manager(
        &self,
        provider: DynProvider,
        service_handler_address: alloy_primitives::Address,
    ) -> Result<IWavsServiceManagerInstance<DynProvider>, AggregatorError> {
        // we need to get the service manager from the perspective of the service handler
        // which may be different than the service manager where the operator is staked
        // e.g. in the case of operator sets that are mirrored across multiple chains
        let service_handler =
            IWavsServiceHandlerInstance::new(service_handler_address, provider.clone());

        let service_manager_address = service_handler
            .getServiceManager()
            .call()
            .await
            .map_err(AggregatorError::EvmServiceManagerLookup)?;

        Ok(IWavsServiceManagerInstance::new(
            service_manager_address,
            provider,
        ))
    }
}
