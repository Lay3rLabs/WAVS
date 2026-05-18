//! WAVS Solana event-relay operator component (v1 trigger demo).
//!
//! Receives a [`TriggerData::SolanaProgramEvent`] from the Solana trigger
//! stream, strips the Anchor `MessageEmitted` framing (8-byte discriminator
//! + 4-byte little-endian length prefix from borsh's `Vec<u8>`), and emits
//! an EVM `DataWithId`-encoded `WasmResponse` so the relayed payload can
//! land in the example `SimpleSubmit` service handler.
//!
//! Why this component is its own crate:
//! - `example_helpers::trigger::decode_trigger_event` only knows about EVM /
//!   Cosmos / ATProto / Hypercore variants today; it rejects
//!   `SolanaProgramEvent`. Rather than add a Solana branch to the shared
//!   helper before there is a worked example, this component does the
//!   Solana-specific framing inline.
//! - The trigger id we hand to the EVM handler is the slot — sufficient
//!   to disambiguate distinct events in the v1 demo and keep `DataWithId`
//!   monotonic across replays. `(slot, signature, instruction_index,
//!   log_index)` replay protection lives in the dispatcher.

use example_helpers::bindings::world::{
    host,
    wavs::{
        operator::{
            input::{TriggerAction, TriggerData},
            output::WasmResponse,
        },
        types::events::TriggerDataSolanaProgramEvent,
    },
    Guest,
};
use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::encode_trigger_output;

struct Component;

/// Anchor event discriminator for `MessageEmitted`:
/// `sha256("event:MessageEmitted")[..8]`.
///
/// Pre-computed at build-time to avoid pulling sha2 into the WASI component.
/// Verified by the `solana_e2e` integration test (see
/// `packages/wavs/tests/solana_e2e.rs::anchor_event_discriminator`).
const MESSAGE_EMITTED_DISCRIMINATOR: [u8; 8] = [0xe4, 0xdc, 0xc4, 0x21, 0x33, 0x5e, 0xc3, 0x35];

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<WasmResponse>, String> {
        let TriggerDataSolanaProgramEvent {
            slot, data: raw, ..
        } = match trigger_action.data {
            TriggerData::SolanaProgramEvent(ev) => ev,
            other => {
                return Err(format!(
                    "solana-event-relay expected SolanaProgramEvent, got {other:?}"
                ));
            }
        };

        let payload = strip_anchor_framing(&raw)?;

        // Use the slot as the trigger id. This is monotonic per-slot
        // and disambiguates distinct events in the v1 demo; full replay
        // protection is enforced at the dispatcher via the
        // (slot, signature, instruction_index, log_index) tuple.
        Ok(vec![encode_trigger_output(
            slot,
            payload,
            host::get_service().service.manager,
        )])
    }
}

/// Strip Anchor `MessageEmitted` framing:
/// - first 8 bytes: discriminator (must match `MESSAGE_EMITTED_DISCRIMINATOR`)
/// - next 4 bytes: little-endian `u32` length prefix from borsh's `Vec<u8>`
/// - remaining bytes: the payload (length must match the prefix)
fn strip_anchor_framing(raw: &[u8]) -> Result<Vec<u8>, String> {
    if raw.len() < 8 {
        return Err(format!(
            "payload too short ({} bytes) — expected at least 8 for the discriminator",
            raw.len()
        ));
    }
    let (disc, rest) = raw.split_at(8);
    if disc != MESSAGE_EMITTED_DISCRIMINATOR {
        return Err(format!(
            "discriminator mismatch: got {disc:02x?}, expected {:02x?}",
            MESSAGE_EMITTED_DISCRIMINATOR
        ));
    }
    if rest.len() < 4 {
        return Err(format!(
            "missing borsh length prefix after discriminator (only {} bytes)",
            rest.len()
        ));
    }
    let (len_bytes, payload) = rest.split_at(4);
    let claimed_len = u32::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
    if claimed_len != payload.len() {
        return Err(format!(
            "borsh length prefix {claimed_len} != actual payload length {}",
            payload.len()
        ));
    }
    Ok(payload.to_vec())
}

export_layer_trigger_world!(Component);
