use alloy_sol_types::SolValue;
use cosmwasm_std::{Addr, Binary, Uint64};
use cw_storage_plus::{Item, Map};
use wavs_types::contracts::cosmwasm::service_handler::{WavsEnvelope, WavsSignatureData};

use crate::solidity_types::example_submit::{DataWithId, SignedData};

pub const SERVICE_MANAGER: Item<Addr> = Item::new("service-manager");

pub const TRIGGER_DATA: Map<Uint64, Binary> = Map::new("trigger-data");

pub fn save_envelope(
    storage: &mut dyn cosmwasm_std::Storage,
    envelope: WavsEnvelope,
    signature_data: WavsSignatureData,
) -> cosmwasm_std::StdResult<()> {
    let envelope = envelope.decode()?;

    let data_with_id = DataWithId::abi_decode(&envelope.payload)
        .map_err(|e| cosmwasm_std::StdError::msg(format!("Failed to decode DataWithId: {e:?}")))?;

    let signed_data = SignedData {
        data: data_with_id.data,
        signatureData: crate::solidity_types::example_submit::IWavsServiceHandler::SignatureData {
            signers: signature_data
                .signers
                .into_iter()
                .map(alloy_primitives::Address::from)
                .collect(),
            signatures: signature_data
                .signatures
                .into_iter()
                .map(|s| s.to_vec().into())
                .collect(),
            referenceBlock: signature_data.reference_block,
        },
        envelope: crate::solidity_types::example_submit::IWavsServiceHandler::Envelope {
            eventId: envelope.eventId,
            ordering: envelope.ordering,
            payload: envelope.payload,
        },
    };

    let signed_data = Binary::from(signed_data.abi_encode());

    let trigger_id = Uint64::from(data_with_id.triggerId);

    TRIGGER_DATA.save(storage, trigger_id, &signed_data)
}
