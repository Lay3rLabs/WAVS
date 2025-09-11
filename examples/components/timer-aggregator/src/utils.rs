use alloy_network::Ethereum;
use alloy_provider::Provider;
use wavs_wasi_utils::evm::new_evm_provider;
use wit_bindgen::block_on;

use crate::world::{
    host,
    wavs::types::events::{TriggerData, TriggerDataEvmContractEvent},
};

pub fn is_valid_tx(trigger_data: TriggerData) -> Result<bool, String> {
    match trigger_data {
        TriggerData::EvmContractEvent(TriggerDataEvmContractEvent { chain, log }) => {
            let chain_config = host::get_evm_chain_config(&chain)
                .ok_or(format!("Could not get evm chain config for {chain}"))?;
            let provider = new_evm_provider::<Ethereum>(
                chain_config
                    .http_endpoint
                    .ok_or(format!("Could not get http endpoint for {chain}"))?,
            );

            let sized_hash: [u8; 32] = log
                .tx_hash
                .try_into()
                .map_err(|_| "Could not convert tx hash to FixedBytes<32>")?;

            let maybe_tx =
                block_on(async move { provider.get_transaction_by_hash(sized_hash.into()).await })
                    .map_err(|e| format!("Could not query transaction via RPC {e}"))?;

            if let Some(tx) = maybe_tx {
                if let Some(block_hash) = tx.block_hash {
                    let sized_hash: [u8; 32] = log
                        .block_hash
                        .unwrap()
                        .try_into()
                        .map_err(|_| "Could not convert block hash to FixedBytes<32>")?;

                    return Ok(block_hash == sized_hash);
                }
            }

            Ok(false)
        }
        _ => Ok(true),
    }
}
