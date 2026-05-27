//! Trigger emission for the in-process tier.
//!
//! The in-process runner uses [`wavs_types::Trigger::Manual`] for its synthetic
//! service, so triggers are driven explicitly by the test rather than picked up
//! from a chain. These helpers build the input bytes the component receives.

use serde::Serialize;

/// Serialize an arbitrary `Serialize` value to JSON bytes for use as a manual
/// trigger payload. Most reference components in `examples/components/` accept
/// JSON-encoded input.
pub fn manual_input_json<T: Serialize>(input: &T) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(input)?)
}

/// Wrap raw bytes as a manual trigger payload (no encoding). Use for components
/// that expect a specific byte layout (e.g. ABI-encoded).
pub fn manual_input_raw(bytes: impl Into<Vec<u8>>) -> Vec<u8> {
    bytes.into()
}
