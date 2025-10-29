use cosmwasm_std::{Addr, Uint64};
use cw_storage_plus::{Item, Map};
use cw_wavs_mock_api::message_with_id::MessageWithId;
use wavs_types::contracts::cosmwasm::service_handler::{WavsEnvelope, WavsSignatureData};

pub const SERVICE_MANAGER: Item<Addr> = Item::new("service-manager");

pub const TRIGGER_MESSAGE: Map<Uint64, String> = Map::new("trigger-message");
pub const SIGNATURE_DATA: Map<Uint64, WavsSignatureData> = Map::new("signature-data");

pub fn save_envelope(
    storage: &mut dyn cosmwasm_std::Storage,
    envelope: WavsEnvelope,
    signature_data: WavsSignatureData,
) -> cosmwasm_std::StdResult<()> {
    let envelope = envelope.decode()?;
    let message_with_id = MessageWithId::from_bytes(&envelope.payload)?;

    TRIGGER_MESSAGE.save(
        storage,
        message_with_id.trigger_id,
        &message_with_id.message,
    )?;
    SIGNATURE_DATA.save(storage, message_with_id.trigger_id, &signature_data)?;

    Ok(())
}
