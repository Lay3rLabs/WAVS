//! Solana program-event trigger emitter for the e2e runner.
//!
//! Mirrors the EVM trigger emitter shape in [`super::runner::Runner`]: it
//! builds and submits a single transaction to a running
//! `solana-test-validator`, then returns the `(slot, signature, log_index)`
//! identity bits the test assertions need.
//!
//! v1 scope: invoke the `event_emitter::emit` instruction in
//! `examples/contracts/solana/event-emitter/`. That instruction takes a
//! single `Vec<u8>` payload and emits an Anchor `MessageEmitted` event,
//! producing a `Program data:` log line that the WAVS Solana trigger
//! stream's `SolanaEventFilter::Discriminator` filter selects on.
//!
//! Why this lives in its own module rather than inline in `runner.rs`:
//! the Solana transaction-construction surface is heavier than the EVM
//! arm (instruction encoding, blockhash refresh, commitment plumbing) and
//! belongs adjacent to the slice 1/2 stream code that documents the
//! framing.

use anyhow::{anyhow, bail, Context, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
pub use solana_keypair::Keypair;
use solana_message::Message;
pub use solana_pubkey::Pubkey;
use solana_sha256_hasher::hash as sha256;
pub use solana_signer::Signer;
use solana_transaction::Transaction;
use std::time::Duration;

/// Result of a single `emit_event` call against the fixture program.
#[derive(Debug, Clone)]
pub struct SolanaEmitOutcome {
    /// Slot the transaction landed in (from `getSignatureStatuses`).
    pub slot: u64,
    /// Transaction signature, base58-encoded.
    pub signature: String,
}

/// Build and submit an `event_emitter::emit(payload)` transaction.
///
/// `rpc_endpoint` is the HTTP endpoint of the running
/// `solana-test-validator` (default `http://127.0.0.1:8899`).
///
/// `program_id` is the deployed program id of the fixture program.
///
/// `payer` is the fee-payer keypair; the test infra is responsible for
/// airdropping it before the call (see `airdrop_payer`).
///
/// The Anchor instruction discriminator for `emit` is
/// `sha256("global:emit")[..8]`. The instruction data layout is:
///
/// ```text
/// [ 8-byte discriminator ][ 4-byte LE length ][ payload bytes ]
/// ```
///
/// Returns once the transaction is `confirmed` (matching the default
/// trigger commitment).
pub async fn emit_event(
    rpc_endpoint: &str,
    program_id: Pubkey,
    payer: &Keypair,
    payload: &[u8],
) -> Result<SolanaEmitOutcome> {
    let client =
        RpcClient::new_with_commitment(rpc_endpoint.to_string(), CommitmentConfig::confirmed());

    let blockhash: Hash = client
        .get_latest_blockhash()
        .await
        .context("getLatestBlockhash failed; is `solana-test-validator` running?")?;

    let instruction = build_emit_instruction(program_id, payer.pubkey(), payload);
    let message = Message::new(&[instruction], Some(&payer.pubkey()));
    let mut tx = Transaction::new_unsigned(message);
    tx.try_sign(&[payer], blockhash)
        .context("failed to sign emit transaction")?;

    let signature = client
        .send_and_confirm_transaction(&tx)
        .await
        .context("send_and_confirm_transaction failed")?;
    let signature_str = signature.to_string();

    // Look up the slot the transaction landed in. We could read it from
    // the confirmed-status response but the explicit lookup keeps the
    // call shape symmetric with the EVM runner's receipt-extraction.
    let slot = client
        .get_transaction_with_config(
            &signature,
            solana_client::rpc_config::RpcTransactionConfig {
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: Some(0),
                encoding: None,
            },
        )
        .await
        .context("get_transaction failed for emit signature")?
        .slot;

    Ok(SolanaEmitOutcome {
        slot,
        signature: signature_str,
    })
}

/// Build the raw [`Instruction`] for `event_emitter::emit(payload)`.
///
/// Exposed for the dispatcher replay-protection regression test, which
/// uses this to materialise the same instruction twice and verify the
/// dispatcher dedups on the replay-identity tuple.
pub fn build_emit_instruction(program_id: Pubkey, signer: Pubkey, payload: &[u8]) -> Instruction {
    let mut data = Vec::with_capacity(8 + 4 + payload.len());
    data.extend_from_slice(&emit_discriminator());
    data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    data.extend_from_slice(payload);

    Instruction {
        program_id,
        accounts: vec![AccountMeta::new_readonly(signer, true)],
        data,
    }
}

/// `sha256("global:emit")[..8]` — Anchor's instruction discriminator
/// for the program's `emit` instruction.
pub fn emit_discriminator() -> [u8; 8] {
    let digest = sha256(b"global:emit");
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest.to_bytes()[..8]);
    out
}

/// Airdrop `lamports` to `payer`, polling until the balance reflects it
/// or `timeout` elapses. The validator's airdrop is normally near-instant
/// but the poll keeps us honest under load.
pub async fn airdrop_payer(
    rpc_endpoint: &str,
    payer: &Pubkey,
    lamports: u64,
    timeout: Duration,
) -> Result<()> {
    let client =
        RpcClient::new_with_commitment(rpc_endpoint.to_string(), CommitmentConfig::confirmed());

    let sig = client
        .request_airdrop(payer, lamports)
        .await
        .context("requestAirdrop failed; is `solana-test-validator` running?")?;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::time::Instant::now() > deadline {
            bail!("airdrop {sig} did not confirm within {:?}", timeout);
        }
        if let Ok(true) = client.confirm_transaction(&sig).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Parse a base58-encoded program id, surfacing a useful error if the
/// caller passed a non-Solana value (e.g. an EVM address that got copied
/// into the wrong slot in service.json).
pub fn parse_program_id(s: &str) -> Result<Pubkey> {
    s.parse::<Pubkey>()
        .map_err(|e| anyhow!("invalid Solana program id '{s}': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_discriminator_matches_anchor_spec() {
        // Anchor instruction discriminator: sha256("global:<instruction>")[..8].
        // This must stay in sync with the fixture program in
        // `examples/contracts/solana/event-emitter/`.
        let disc = emit_discriminator();
        assert_eq!(disc.len(), 8);
        // Computed independently from `sha256("global:emit")`:
        // 0xde 0x35 0x14 0xaf 0x14 0x57 0x05 0xc5
        // The exact value isn't load-bearing for the test — what
        // matters is that the function is deterministic and the runner
        // and fixture agree on it.
        let expected = sha256(b"global:emit");
        assert_eq!(&disc[..], &expected.to_bytes()[..8]);
    }

    #[test]
    fn build_emit_instruction_data_layout() {
        let program = Pubkey::new_unique();
        let signer = Pubkey::new_unique();
        let payload = b"hello-svm";

        let ix = build_emit_instruction(program, signer, payload);
        assert_eq!(ix.program_id, program);
        assert_eq!(ix.accounts.len(), 1);
        assert_eq!(ix.accounts[0].pubkey, signer);
        assert!(ix.accounts[0].is_signer);
        assert!(!ix.accounts[0].is_writable);

        // Data: discriminator || u32-le length || payload
        assert_eq!(ix.data.len(), 8 + 4 + payload.len());
        assert_eq!(&ix.data[..8], &emit_discriminator());
        let len = u32::from_le_bytes(ix.data[8..12].try_into().unwrap());
        assert_eq!(len as usize, payload.len());
        assert_eq!(&ix.data[12..], payload);
    }
}
