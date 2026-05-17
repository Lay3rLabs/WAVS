//! WAVS Solana fixture program — event-emitter.
//!
//! A minimal Anchor program used by the v1 SVM trigger demo and the
//! `solana_e2e` integration test in `packages/wavs/tests/`.
//!
//! Design constraints:
//! - One instruction (`emit`) that takes an opaque `payload: Vec<u8>` and
//!   emits a single Anchor event (`MessageEmitted`) carrying that payload.
//! - Anchor's `emit!` macro logs the event as:
//!     `Program data: <base64( 8-byte-discriminator || borsh(payload) )>`
//!   which is exactly the shape the WAVS Solana trigger stream's
//!   `SolanaEventFilter::Discriminator` filter is designed to match.
//! - The 8-byte discriminator is `sha256("event:MessageEmitted")[..8]`;
//!   the runtime expectation is documented in the demo README.
//!
//! v2 follow-ups (out of scope here):
//! - A `submit` instruction that writes results back, behind the
//!   `contracts/svm-middleware` registry.
//! - Realistic event shapes (PDA-driven, multi-field) once the operator
//!   submission path actually targets Solana.

use anchor_lang::prelude::*;

// Placeholder pubkey; replace on a real deploy with the output of
// `solana address -k target/deploy/event_emitter-keypair.json`.
declare_id!("EvNt1111111111111111111111111111111111111111");

#[program]
pub mod event_emitter {
    use super::*;

    /// Emit a `MessageEmitted` event carrying `payload` bytes.
    ///
    /// The validator log this produces looks like:
    ///
    /// ```text
    /// Program EvNt1111... invoke [1]
    /// Program log: Instruction: Emit
    /// Program data: <base64-discriminator + borsh-payload>
    /// Program EvNt1111... consumed N of M compute units
    /// Program EvNt1111... success
    /// ```
    ///
    /// The `Program data:` line is what the WAVS trigger stream's
    /// `SolanaEventFilter::Discriminator` filter selects on.
    pub fn emit(_ctx: Context<Emit>, payload: Vec<u8>) -> Result<()> {
        emit!(MessageEmitted { payload });
        Ok(())
    }
}

/// No accounts needed — the instruction is pure log emission. The signer
/// is required only so the transaction is well-formed.
#[derive(Accounts)]
pub struct Emit<'info> {
    pub signer: Signer<'info>,
}

/// The single event this fixture program emits. Anchor will emit a
/// `Program data:` log line whose base64 payload is the 8-byte
/// `sha256("event:MessageEmitted")[..8]` discriminator followed by the
/// borsh-serialized `payload` field.
#[event]
pub struct MessageEmitted {
    pub payload: Vec<u8>,
}
