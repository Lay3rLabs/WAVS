//! Signed-envelope helpers verified against the canonical WAVS handler shape.
//!
//! The handler ABI is defined by `IWavsServiceHandler` from `@wavs/solidity`:
//!
//! ```solidity
//! struct Envelope { bytes20 eventId; bytes12 ordering; bytes payload; }
//! struct SignatureData { address[] signers; bytes[] signatures; uint32 referenceBlock; }
//! ```
//!
//! The on-chain validator (`WavsServiceManager.validate`) computes
//! `message = keccak256(abi.encode(envelope))`, applies the standard
//! `\x19Ethereum Signed Message:\n32` prefix, and checks each operator
//! signature against the resolved stake registry.
//!
//! This module mirrors that exact shape so harness-produced envelopes verify on
//! real handlers without modification. The Solidity sources of truth are bundled
//! with downstream apps under `@wavs/solidity@0.6.x`.

use alloy_primitives::{eip191_hash_message, keccak256, Address, FixedBytes, B256, U256};
use alloy_provider::Provider;
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::{sol, SolCall, SolValue};
use anyhow::{anyhow, Context, Result};

sol! {
    /// The canonical envelope shape that all WAVS service handlers verify.
    #[derive(Debug)]
    struct Envelope {
        bytes20 eventId;
        bytes12 ordering;
        bytes payload;
    }

    /// The canonical signature data accompanying every envelope.
    #[derive(Debug)]
    struct SignatureData {
        address[] signers;
        bytes[] signatures;
        uint32 referenceBlock;
    }

    /// `IWavsServiceHandler.handleSignedEnvelope` selector, used to encode
    /// on-chain submission calldata.
    #[sol(rpc)]
    interface IHandler {
        function handleSignedEnvelope(
            Envelope calldata envelope,
            SignatureData calldata signatureData
        ) external;
    }
}

impl Envelope {
    /// Build an envelope with empty ordering padding (the field is reserved for
    /// future ordering semantics — handlers ignore it today).
    pub fn new(event_id: [u8; 20], payload: impl Into<Vec<u8>>) -> Self {
        Self {
            eventId: FixedBytes::from(event_id),
            ordering: FixedBytes::default(),
            payload: payload.into().into(),
        }
    }

    /// Compute the keccak256(abi.encode(envelope)) digest the on-chain validator
    /// uses as input to the EIP-191 personal-sign prefix.
    pub fn message_hash(&self) -> B256 {
        keccak256(self.abi_encode())
    }

    /// Compute the EIP-191 `\x19Ethereum Signed Message:\n32` digest the operator
    /// signers actually sign.
    pub fn signing_hash(&self) -> B256 {
        eip191_hash_message(self.message_hash())
    }
}

/// Sign an envelope with a set of operator signers and produce the matching
/// [`SignatureData`].
///
/// `reference_block` must be strictly less than `block.number` at validation time;
/// callers typically pass the current block number minus one.
pub fn sign_envelope(
    envelope: &Envelope,
    signers: &[PrivateKeySigner],
    reference_block: u32,
) -> Result<SignatureData> {
    if signers.is_empty() {
        return Err(anyhow!("at least one signer required"));
    }
    let hash = envelope.signing_hash();
    let mut signer_addrs = Vec::with_capacity(signers.len());
    let mut sigs = Vec::with_capacity(signers.len());
    for s in signers {
        let sig = s
            .sign_hash_sync(&hash)
            .map_err(|e| anyhow!("sign envelope: {e}"))?;
        signer_addrs.push(s.address());
        sigs.push(sig.as_bytes().to_vec().into());
    }
    Ok(SignatureData {
        signers: signer_addrs,
        signatures: sigs,
        referenceBlock: reference_block,
    })
}

/// Build a 20-byte event id from a 32-byte seed (typically a transaction hash or
/// content-addressed id). Takes the low 20 bytes — matches WAVS' convention.
pub fn event_id_from_seed(seed: B256) -> [u8; 20] {
    let mut out = [0u8; 20];
    out.copy_from_slice(&seed.as_slice()[12..]);
    out
}

/// Build a 20-byte event id from a `u64` nonce. Useful in tests where the
/// envelope sequence is driven by the test rather than chain state.
pub fn event_id_from_nonce(nonce: u64) -> [u8; 20] {
    let mut seed = [0u8; 32];
    seed[24..].copy_from_slice(&nonce.to_be_bytes());
    event_id_from_seed(B256::from(seed))
}

/// Convenience: u256 abi-encoded payload (rare — most apps have a struct payload).
pub fn encode_u256_payload(v: U256) -> Vec<u8> {
    v.abi_encode()
}

/// Returns the resolved signer address for a signer (useful when wiring into
/// the operator registry).
pub fn signer_address(signer: &PrivateKeySigner) -> Address {
    signer.address()
}

/// Submit a signed envelope to a service handler's `handleSignedEnvelope`
/// entry point. Returns the transaction receipt.
///
/// This is the "submit" stage of the WAVS lifecycle, completing the
/// `trigger → compute → aggregate → sign → SUBMIT → assert` path. Use
/// [`crate::service::handler::MockHandler::deploy`] to deploy the contract on
/// local Anvil, then pass `handler.handler` as the `handler_addr`.
///
/// The function uses ABI-encoded calldata with the canonical
/// `IWavsServiceHandler.handleSignedEnvelope(Envelope, SignatureData)`
/// selector, so the same call shape works against any compliant handler —
/// `SimpleSubmit`, `wavs-defi`'s `SmartVaultServiceHandler`, or production
/// handlers built on `@wavs/solidity`.
pub async fn submit_envelope<P>(
    provider: &P,
    handler_addr: Address,
    envelope: &Envelope,
    signature: &SignatureData,
) -> Result<alloy_rpc_types_eth::TransactionReceipt>
where
    P: Provider,
{
    use alloy_network::TransactionBuilder;
    use alloy_rpc_types_eth::TransactionRequest;

    let calldata = IHandler::handleSignedEnvelopeCall {
        envelope: envelope.clone(),
        signatureData: signature.clone(),
    }
    .abi_encode();

    let tx = TransactionRequest::default()
        .with_to(handler_addr)
        .with_input(calldata);

    let pending = provider
        .send_transaction(tx)
        .await
        .context("send handleSignedEnvelope tx")?;
    let receipt = pending
        .get_receipt()
        .await
        .context("await handleSignedEnvelope receipt")?;
    if !receipt.status() {
        return Err(anyhow!(
            "handleSignedEnvelope reverted (tx={:?})",
            receipt.transaction_hash
        ));
    }
    tracing::debug!(
        tx = ?receipt.transaction_hash,
        block = ?receipt.block_number,
        gas = receipt.gas_used,
        "envelope submitted"
    );
    Ok(receipt)
}

/// Sort signers (and their signatures) ascending by address — required by
/// `SimpleServiceManager` and `WavsServiceManager.validate()`. Mutates in place.
///
/// Call this *after* [`sign_envelope`] but *before* submitting if signers were
/// not registered in sorted order.
pub fn sort_signature_data(sigdata: &mut SignatureData) {
    let mut indices: Vec<usize> = (0..sigdata.signers.len()).collect();
    indices.sort_by_key(|i| sigdata.signers[*i]);
    let new_signers: Vec<Address> = indices.iter().map(|i| sigdata.signers[*i]).collect();
    let new_sigs: Vec<alloy_primitives::Bytes> = indices
        .iter()
        .map(|i| sigdata.signatures[*i].clone())
        .collect();
    sigdata.signers = new_signers;
    sigdata.signatures = new_sigs;
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Bytes;

    #[test]
    fn envelope_round_trip_abi_encode() {
        let event_id = event_id_from_nonce(42);
        let payload: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef];
        let env = Envelope::new(event_id, payload.clone());

        // The encoded envelope should embed the payload and event id.
        let encoded = env.abi_encode();
        assert!(encoded.len() > 32 + 12 + payload.len());
    }

    #[test]
    fn signing_hash_differs_from_message_hash() {
        let env = Envelope::new(event_id_from_nonce(1), vec![1, 2, 3]);
        let msg = env.message_hash();
        let signed = env.signing_hash();
        assert_ne!(msg, signed, "EIP-191 prefix must change the digest");
    }

    #[test]
    fn sign_envelope_produces_recoverable_signature() {
        let signer = PrivateKeySigner::random();
        let env = Envelope::new(event_id_from_nonce(7), vec![0xaa]);
        let sigdata = sign_envelope(&env, &[signer.clone()], 0).unwrap();

        assert_eq!(sigdata.signers.len(), 1);
        assert_eq!(sigdata.signers[0], signer.address());
        assert_eq!(sigdata.signatures.len(), 1);

        // Sanity: signature is 65 bytes (r || s || v).
        let sig_bytes: &Bytes = &sigdata.signatures[0];
        assert_eq!(sig_bytes.len(), 65);
    }

    #[test]
    fn empty_signer_list_errors() {
        let env = Envelope::new(event_id_from_nonce(1), vec![]);
        let res = sign_envelope(&env, &[], 0);
        assert!(res.is_err());
    }

    #[test]
    fn sort_signature_data_sorts_signers_and_signatures_together() {
        // Build a SignatureData with intentionally out-of-order signers, then
        // sort and check both arrays moved together.
        let a = Address::from([0xaa; 20]);
        let b = Address::from([0x11; 20]);
        let c = Address::from([0x55; 20]);

        let mut sd = SignatureData {
            signers: vec![a, b, c],
            signatures: vec![
                alloy_primitives::Bytes::from_static(b"sig-aa"),
                alloy_primitives::Bytes::from_static(b"sig-11"),
                alloy_primitives::Bytes::from_static(b"sig-55"),
            ],
            referenceBlock: 0,
        };
        sort_signature_data(&mut sd);

        assert_eq!(sd.signers, vec![b, c, a]);
        assert_eq!(sd.signatures[0].as_ref(), b"sig-11");
        assert_eq!(sd.signatures[1].as_ref(), b"sig-55");
        assert_eq!(sd.signatures[2].as_ref(), b"sig-aa");
    }
}
