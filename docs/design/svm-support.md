# Solana / SVM Support — Design Doc

**Status:** Draft, 2026-05-17
**Tracking issue:** [Lay3rLabs/WAVS#1149](https://github.com/Lay3rLabs/WAVS/issues/1149)
**Scope:** Boundaries and v1 scope. Not a spec — does not define PDAs, WIT diffs, or program layouts.

## Summary

WAVS supports EVM and Cosmos chains today. This doc proposes making Solana a first-class chain alongside them, in two phases:

- **v1 — trigger-only.** A Solana program event triggers a WAVS component; submission happens through the existing EVM or Cosmos paths. No on-chain Solana writes, no middleware program. Validates abstraction boundaries before committing to the harder half.
- **v2 — full bidi.** Solana submission path, middleware program (signature verification + operator registry + replay protection on-program), and a `wavs-anchor-template`. Open decisions on signature scheme, operator-set shape, and Anchor-vs-native are listed but not resolved in this doc.

The v1/v2 split is deliberate. Trigger-only is roughly one-third of the work and unlocks "Solana event → EVM handler" demos that prove the design without on-chain Solana write infrastructure.

## Context

Issue #1149 asks for SVM support that is **a peer of EVM and Cosmos, not an overload of either.** Eight implementation areas are called out: chain config, trigger source, submission target, service-manager program, operator signing/quorum, WIT/SDK exposure, CLI/service-JSON/UX, and a test harness against `solana-test-validator`. The acceptance criteria in the issue all sit in the trigger half except the service-manager program and the submission path.

The repo already contains an empty toehold: `contracts/svm-middleware/` is an Anchor 0.32.1 scaffold with a single empty `initialize` instruction (`programs/svm-middleware/src/lib.rs`, 16 lines) and a `PLAN.md` (7 lines) committing to "two programs, BLS + ECDSA, mirroring `cw-middleware` and `poa-middleware`." Nothing in `WAVS/packages/` references Solana, SVM, or Anchor today. The only `ed25519` mentions are "future:" comments in `packages/types/src/service.rs` (around lines 524 and 560).

## Abstraction Boundaries

These are the seams in WAVS core where chain type forks today. Every one needs a Solana arm in either v1 or v2.

| Concern | File | What forks today |
|---|---|---|
| Chain config | `packages/types/src/chain_config.rs:85` | `AnyChainConfig::{Cosmos, Evm}` |
| Service manager | `packages/types/src/service.rs:102` | `ServiceManager::{Evm, Cosmos}` |
| Trigger (declared) | `packages/types/src/service.rs` | `Trigger::{EvmContractEvent, CosmosContractEvent, BlockInterval, Cron, AtProtoEvent, HypercoreAppend, Manual}` |
| TriggerData (delivered) | `packages/types/src/service.rs:446` | mirror of the above, plus `Raw` |
| Signature kind | `packages/types/src/service.rs:558` | `SignatureAlgorithm::Secp256k1`, `SignaturePrefix::Eip191` only |
| Submission path | `packages/wavs/src/subsystems/submission.rs` | EVM-signer only (`MissingEvmSigner` at :161, `FailedToCreateEvmSigner` at :259) |
| Trigger streams | `packages/wavs/src/subsystems/trigger/streams/` | `evm_stream.rs`, `cosmos_stream.rs`, `atproto_jetstream.rs`, `hypercore_stream.rs`, `cron_stream.rs`, `local_command_stream.rs` |
| Engine ↔ component bindings | `packages/engine/src/bindings/types/` | mirrors the type enums |
| WIT | `wit-definitions/types/wit/{service,events,chain}.wit` | `service-manager`, `trigger`, `trigger-data`, address types |
| CLI service JSON validation | `packages/cli/src/service_json.rs` | per-variant matches |
| WASI helper utils | `packages/wasi-utils/src/evm/` | EVM-only `event.rs`, `provider.rs` |

Two things to note:

1. The trigger surface is well-trodden. Non-chain triggers (`Cron`, `AtProtoEvent`, `HypercoreAppend`) already live next to the EVM/Cosmos triggers, so adding a Solana trigger source does not require carving new abstraction — it follows an existing pattern.
2. The submission surface is **single-target and EVM-shaped.** `submission.rs` ties the signer type to EVM. v2 will need a Solana submitter alongside it.

## v1 — Trigger-Only Solana Support

### Goal

A WAVS service can declare a Solana chain and a Solana program-event trigger. When the configured Solana program emits the matching event at the configured commitment level, WAVS observes it exactly once, hands typed trigger data to the operator component via WIT bindings, and the operator submits back through an existing EVM or Cosmos service manager.

The "exactly once" property is enforced by a trigger identity composed of `(slot, signature, instruction_index, inner_instruction_index, log_index)` — sufficient to dedupe across reorgs at the configured commitment.

### Work Breakdown (inventory only — no code in this doc)

- **Types** — add `AnyChainConfig::Solana(SolanaChainConfig)`, `Trigger::SolanaProgramEvent { chain, program_id, event_discriminator | log_filter, commitment }`, and matching `TriggerData::SolanaProgramEvent { chain, slot, signature, instruction_index, inner_instruction_index, log_index, data }`. Extend `type_name` and `chain()` accessors.
- **Trigger stream** — new `solana_stream.rs` peer to `evm_stream.rs` and `cosmos_stream.rs`, using `solana_client::nonblocking::pubsub_client::PubsubClient` (`logs_subscribe`, optionally `program_subscribe` for state-change triggers in a later iteration). Commitment is a config knob, default `confirmed`.
- **Lookup** — extend `subsystems/trigger/lookup.rs` to key Solana triggers by `(chain, program_id, event_discriminator)`.
- **WIT** — add `solana-address`/`pubkey` type in `chain.wit`, a `solana-program-event` record in `events.wit`, and the corresponding `trigger`/`trigger-data` variants in `service.wit`. Regenerate component bindings.
- **CLI** — branch in `service_json.rs` validators for the new variant; surface in service-JSON examples.
- **Dev loop** — `solana-test-validator` wired into a `justfile` target alongside `start-anvil`. A worked example (Solana program → WAVS component → EVM submission) that doubles as the integration test.

### Acceptance Criteria

1. A service.json can declare `chain: solana`, configure RPC/commitment, and a `SolanaProgramEvent` trigger.
2. A test fixture program running on `solana-test-validator` emits an event; the WAVS node observes it.
3. The matching operator component receives typed `TriggerData::SolanaProgramEvent` via WIT bindings.
4. Replay protection: the same `(slot, signature, instruction_index, log_index)` does not re-fire the operator. Verified by deliberately reconnecting the subscription.
5. Submission to an EVM service manager from the operator works end-to-end.
6. `just` target spins up `solana-test-validator` + WAVS + the EVM submission target locally.

### Non-Goals for v1

- No Solana submission target. Operators must submit to EVM or Cosmos.
- No Solana middleware program work beyond what is already in `contracts/svm-middleware/`.
- No Solana operator key support. Operators continue to sign with their EVM key; the trigger doesn't care.
- No reorg recovery beyond what `confirmed`/`finalized` commitment provides. Reorgs above the chosen commitment are treated as user error.
- No `programSubscribe`/state-change triggers in v1 — log/event triggers only. State triggers are a v1.5 follow-up if demand exists.

## v2 Sketch — Submission + Middleware Program

This section is non-binding. It frames the work so v1 boundaries don't paint v2 into a corner.

### Submission

A `solana_submitter` peer to the EVM/Cosmos paths in `submission.rs`. Materially different mechanics from EVM:

- Recent-blockhash refresh and expiry retry loop
- Compute budget + priority fee instructions
- Optional durable nonce account for high-latency submissions
- Idempotency must be enforced **on the program** (replay PDA) — Solana has no equivalent to EVM nonce-as-ordering
- New credential env var (`WAVS_AGGREGATOR_SOLANA_CREDENTIAL`)
- Fee-payer key separate from signing key, by convention

### Middleware Program

Today's `contracts/svm-middleware/` is a placeholder. The v2 program needs, at minimum: service-config PDA, operator/quorum registry PDA(s), submission replay PDA, a `submit` instruction that verifies aggregated signatures against the registry, and an upgrade-authority policy.

Mirror reference: `contracts/cw-middleware/` and its Mock / ECDSA / BLS triad. The cleanest path likely follows the same progression: ship a POA variant first (operator addresses set by an admin key), then add signature-verifying variants once the operator-set shape is settled.

### Operator Signing — Three Options, One Recommendation

| Option | Pros | Cons |
|---|---|---|
| **Mirror EVM keys (ECDSA)** | Shared operator set across all chains. Works via `secp256k1_recover` syscall. | Operators sign for Solana with a non-native key — awkward UX. |
| **Native Ed25519** | Idiomatic on Solana, cheap on-chain verify via the Ed25519 sig-verify program. | Forks operator identity. Operators have to manage a second key. |
| **BLS aggregate** | Constant-size aggregate verification. | On Solana, BLS verification is hard — needs precompile or a custom verifier. Likely v3 territory. |

**Recommended default:** start with POA (admin-managed operator set, no on-program signature verification — operator membership is the only check). Add ECDSA verification next, mirroring `poa-middleware → wavs-middleware → cw-middleware` progression. Defer BLS until there is a concrete need.

This recommendation should be re-evaluated when v2 kicks off — it's the cheapest path to a working end-to-end demo but it is not the right answer for production multi-chain operator sets, which want either ECDSA-shared or BLS-aggregate.

### Anchor vs. Native

`contracts/svm-middleware/` is Anchor 0.32.1 today. Switching to native means rewriting the toehold. Recommendation: **stay on Anchor for v2 unless we hit a concrete blocker** — the existing scaffold, audit familiarity, and `wavs-anchor-template` future make Anchor the lower-risk choice.

## Open Decisions

Listed with recommended defaults so v1 isn't blocked. Each decision is revisitable before v2 kickoff.

| # | Decision | Recommended default | Revisit before |
|---|---|---|---|
| 1 | v1 scope: trigger-only vs. full bidi | **Trigger-only** | — (this doc decides) |
| 2 | Trigger commitment level | `confirmed` for triggers, `finalized` for v2 submission | v1 implementation |
| 3 | Anchor vs. native program | Anchor | v2 kickoff |
| 4 | Signature scheme | POA first, then ECDSA-mirrored, then BLS | v2 kickoff |
| 5 | Operator registry shape | Mirror cw-middleware progression (Mock/ECDSA/BLS) | v2 kickoff |
| 6 | Driving example | "Solana event → EVM handler" demo | v1 kickoff |
| 7 | State-change triggers (`programSubscribe`) | Deferred to v1.5 | v1.5 if demand exists |

## Non-Goals for This Doc

- Does not specify WIT diffs. v1 implementation owns the exact records and field names.
- Does not specify PDA layouts or program instructions. v2 design memo owns this.
- Does not commit to a final signature scheme. v2 design memo owns this.
- Does not specify operator key onboarding flow. Follows from the signature-scheme decision.
- Does not pick a Solana cluster strategy for hosted demos. Local `solana-test-validator` is sufficient for v1.

## References

- Issue: https://github.com/Lay3rLabs/WAVS/issues/1149
- Existing scaffold: `contracts/svm-middleware/programs/svm-middleware/src/lib.rs`, `contracts/svm-middleware/PLAN.md`
- Mirror references: `contracts/poa-middleware/` (audited, simplest), `contracts/cw-middleware/` (Mock/ECDSA/BLS triad)
- Trigger-stream patterns to copy from: `packages/wavs/src/subsystems/trigger/streams/evm_stream.rs`, `cosmos_stream.rs`, `atproto_jetstream.rs`
- Boundary files: `packages/types/src/service.rs`, `packages/types/src/chain_config.rs`, `packages/wavs/src/subsystems/submission.rs`, `wit-definitions/types/wit/{service,events,chain}.wit`
