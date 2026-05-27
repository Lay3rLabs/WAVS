# wavs-test-harness

Reusable integration test harness for WAVS apps.

Tracks issue [Lay3rLabs/WAVS#1147](https://github.com/Lay3rLabs/WAVS/issues/1147).

## What this crate is for

Downstream app repos — `wavs-defi`, `wavs-aave-guardian`, `wavs-prediction-market`,
and the rest — write integration tests that drive the full WAVS path:

```
trigger → operator component → aggregator (sig + quorum) → service handler → app contract
```

`wavs-test-harness` provides chain control, service-lifecycle, envelope
helpers, and waiters so each downstream test only has to specify its
app-specific contract deployment and assertions.

## Quickstart — local Anvil

```rust
use wavs_test_harness::{chain, service::{InProcRunner, ServiceSpec}};

let (provider, anvil) = chain::spawn_local().await?;

let spec = ServiceSpec::new()
    .component_wasm("path/to/strategy.wasm")
    .aggregator_wasm("path/to/aggregator.wasm")
    .config_var("CHAINLINK_ETH_USD", chainlink.to_string());

let runner = InProcRunner::from_spec(&spec)?;
let outputs = runner.run_component(b"hello".to_vec()).await?;
```

Two runnable examples ship with the crate:

```bash
cargo run -p wavs-test-harness --example minimal_local
FORK_RPC_URL=<base-rpc> cargo run -p wavs-test-harness --example fork_with_addresses
```

## Quickstart — pinned fork

```rust
use wavs_test_harness::{chain, fixtures::ChainProfile};

let profile = ChainProfile::load("base")?;            // chain_id + addresses + rpc_env
// `from_profile` resolves FORK_RPC_URL via the profile's rpc_env and pins the
// block to FORK_BLOCK_NUMBER (env) or profile.chain.fork_block (fallback).
let opts = chain::ForkOptions::from_profile(&profile)?;
let (provider, _anvil) = chain::spawn_fork(opts).await?;

let usdc = profile.address("usdc")?;
let weth = profile.address("weth")?;
let whale = profile.accounts.address("usdc_whale")?;

chain::impersonate_funded(&provider, whale).await?;
// … your test …
chain::stop_impersonating(&provider, whale).await?;
```

### Fork block pinning precedence

For determinism, the harness pins the fork block in this order (highest wins):

1. `ForkOptions::with_block_number(b)` — explicit override.
2. `FORK_BLOCK_NUMBER` env var — CI override.
3. `ChainProfile.chain.fork_block` — profile default (via `from_profile`).
4. None — Anvil follows upstream `latest`; spawn logs a `warn!` so the
   determinism gap is visible.

The RPC URL is never logged verbatim — only via [`chain::redact_url`].

## Tier matrix

| Tier | Stages run | Stages mocked | Use case |
|---|---|---|---|
| **InProc** (default) | compute (real WASM via `wavs-engine`) + aggregate (real WASM via `wavs-engine`) + envelope signing + on-chain `handleSignedEnvelope` + state assertion | dispatcher subsystems | deterministic PR tier on local Anvil or fork |
| **Subprocess** | binary spawn + `/health` probe + `POST /dev/services` + on-chain submission + assert; v1 covers the spawn + service-registration loop, full trigger emission via the harness's `chain::*` primitives is composable | nothing — runs the real `wavs` binary | nightly fork + pre-release |

InProc executes the actual operator **and aggregator** WASM directly through
`wavs-engine` — the `lifecycle_component_through_aggregator_then_sign_then_submit_then_assert`
test in `tests/end_to_end_smoke.rs` drives the full issue-#1147 path:

```
trigger → operator (real WASM) → aggregator (real WASM, emits SubmitAction)
        → harness signs envelope → handleSignedEnvelope on-chain
        → handler state assertion
```

The harness signs after the aggregator emits its `SubmitAction`, matching
production semantics: `packages/wavs/src/subsystems/submission.rs:140-155`
shows the dispatcher building the `Envelope` from the operator's
`WasmResponse` + the aggregator-derived `EventId` and signing it before
submission. `Envelope::from_operator_response` mirrors that conversion
(including the `ordering` field's big-endian u64 packing).

`simple_aggregator.wasm` is pass-through — it always emits one `Submit`
per packet, no quorum check. A multi-operator quorum smoke test belongs
in a follow-up with a quorum-aware aggregator component. The current
test exercises the aggregator **lifecycle stage**, not its quorum
decision-making.

Only the WAVS dispatcher's trigger streaming and submission
orchestration is bypassed — the harness drives each stage explicitly so
tests stay deterministic.

Subprocess spawns the actual `wavs` binary against a tempdir HOME/DATA and
a generated `wavs.toml`, polls `/health` until ready, and exposes the
HTTP-API surface for service registration and lifecycle drive. The runner
is feature-gated behind `subprocess` to keep the default dep graph small.

### End-to-end lifecycle path (InProc, including aggregator)

```rust
use wavs_test_harness::{
    chain,
    envelope::{sign_envelope, Envelope},
    service::{
        InProcRunner, MockHandler, MockHandlerConfig, RunnerAggregatorAction,
        RunnerSubmitAction, ServiceSpec,
    },
};
use wavs_types::AggregatorInput;

// 1. Anvil with a wallet-bound provider.
let (provider, anvil, _deployer) = chain::spawn_local_with_deployer().await?;

// 2. Deploy SimpleServiceManager + SimpleSubmit with one registered operator.
let operator = alloy_signer_local::PrivateKeySigner::random();
let handler = MockHandler::deploy(
    provider.clone(),
    &MockHandlerConfig::single_operator(operator.address()),
).await?;

// 3. Build the spec WITH chain configs + aggregator config vars. The
//    aggregator reads `chain` + `service_handler` from host::config_var and
//    fails closed if `evm:local` is not registered in chain_configs.
let spec = ServiceSpec::new()
    .component_wasm("path/to/strategy.wasm")
    .aggregator_wasm("path/to/simple_aggregator.wasm")
    .with_evm_local_chain("local", &anvil.endpoint())
    .config_var("chain", "evm:local")
    .config_var("service_handler", handler.handler.to_string());
let runner = InProcRunner::from_spec(&spec)?;

// 4. Operator stage — keep the trigger_action so we can reuse it for
//    aggregator-side EventId derivation.
let trigger = runner.default_trigger_action(payload_bytes.clone());
let mut responses = runner.run_component_full(trigger.clone()).await?;
let operator_response = responses.remove(0);

// 5. Aggregator stage — drives simple_aggregator.wasm.
let agg_input = AggregatorInput {
    trigger_action: trigger,
    operator_response: operator_response.clone(),
};
let event_id = agg_input.event_id()?;
let actions = runner.run_aggregator(event_id.clone(), agg_input).await?;
match &actions[0] {
    RunnerAggregatorAction::Submit(RunnerSubmitAction::Evm(evm)) => {
        assert_eq!(&evm.address.raw_bytes[..], handler.handler.as_slice());
    }
    _ => panic!("expected Evm Submit"),
}

// 6. Sign envelope built from the aggregator-derived EventId.
let env = Envelope::from_operator_response(event_id, &operator_response);
let sigdata = sign_envelope(&env, std::slice::from_ref(&operator), reference_block)?;

// 7. Submit + assert.
handler.submit_envelope(&env, &sigdata).await?;
assert!(handler.is_valid_trigger(trigger_id).await?);
```

### Oracle mocking on a fork

```rust
use wavs_test_harness::chain::oracle::{chainlink_usd, install_chainlink_aggregator_v3};

let chainlink_eth_usd = profile.address("chainlink_eth_usd")?;
install_chainlink_aggregator_v3(&provider, chainlink_eth_usd, chainlink_usd(3_000.0)).await?;
// Downstream contract reads now see ETH/USD = $3000 deterministically.
```

## CI tiers

| Tier | Runner | When | Command |
|---|---|---|---|
| **PR deterministic** | local Anvil only | every PR | `just test-harness` |
| **PR labeled fork** | pinned Base / mainnet fork | PRs labeled `fork-tests` | `just test-harness-fork` |
| **Nightly fork matrix** | broader protocol matrix | scheduled | TBD — wired alongside Subprocess landing |
| **Pre-release subprocess** | full `wavs` binary | release branches | TBD — depends on Subprocess landing |

Fork tier requires `FORK_RPC_URL` set in the CI secret store. The harness
never logs the URL; the secret should be scoped to fork-tier jobs only.

## Bundled chain profiles

Three profiles ship via `include_str!`:

- `local.toml`     — `chain_id=31337`, no fork, no env vars.
- `base.toml`      — `chain_id=8453`, `FORK_RPC_URL`, complete protocol address set
                     (Aerodrome pool / NFT manager / router, Avantis trading / storage / gov,
                     Chainlink, Pyth, USDC, WETH, USDC whale, Avantis operator).
- `mainnet.toml`   — `chain_id=1`, `FORK_RPC_URL`, tokens + Aave v3 pool.

Consumers can ship their own profiles via `ChainProfile::from_path(...)`
or `ChainProfile::from_toml_str(...)`.

## Determinism boundary

- **Block-time control.** `chain::set_automine(false)` + `chain::mine_blocks(n)`
  gives deterministic block production. Pair with `chain::set_next_block_timestamp`
  for tests that depend on block timestamps.
- **Snapshot / revert.** `chain::SnapshotGuard::take(&provider)` captures
  state; explicit `.revert(&provider)` rolls back. The guard panics if
  dropped without explicit revert (async-drop is unstable).
- **Operator signing.** All envelope signatures are deterministic for a given
  signer key. Use the same operator set across re-runs to keep signature
  bytes stable.

## Tier selection in downstream repos

Add `wavs-test-harness` as a `[dev-dependencies]` entry only, never as a
regular dependency. This keeps `utils/test-utils` out of production builds.

```toml
[dev-dependencies]
wavs-test-harness = { git = "https://github.com/Lay3rLabs/WAVS", rev = "<commit-sha>" }
```

**Branch references are not supported in CI — pin to a commit SHA.**

### Cross-repo alloy version barrier

When the downstream repo uses a different alloy major version than the WAVS
workspace pin (`1.0.42` at the time of writing), cargo workspace feature
unification can produce compile errors in `alloy-rpc-types-eth` (e.g.
`BlobTransactionSidecarVariant` vs `BlobTransactionSidecar` mismatch).
Resolution paths:

1. Bump alloy in WAVS to match the downstream consumer.
2. Pin the downstream test crate to the WAVS alloy version.
3. Publish `wavs-test-harness` as a versioned crate so cargo handles
   version-mixing instead of unification.

Tracked as the highest-priority follow-up under #1147.

## Layer-tests relationship

`packages/layer-tests` continues to run as-is. `wavs-test-harness` is
**canonical** for new integration tests; `layer-tests` is treated as legacy
and is expected to migrate onto the harness in a follow-up. Two parallel
test crates drift over time, so migration shouldn't sit forever — but it
also doesn't block this v1.

## What ships in v1

- `chain`: local Anvil (`spawn_local`, `spawn_local_with_deployer`) +
  pinned fork (feature `fork`, on by default).
  `snapshot` / `revert` with RAII guard. `impersonate_funded`,
  `enable_auto_impersonate`, `whale_fund`. `mine_blocks`, `set_automine`,
  `increase_time`, `set_next_block_timestamp`. `redact_url` / `redact_key`
  for sanitized logs. `oracle::install_chainlink_aggregator_v3` +
  `set_chainlink_price` for forked-chain oracle mocking.
- `fixtures`: `ChainProfile` (TOML), `Addresses` typed lookup. Three
  bundled profiles. `ForkOptions::from_env` reads `FORK_RPC_URL` +
  `FORK_BLOCK_NUMBER`; `ForkOptions::from_profile` adds the profile's
  declared `fork_block` as a fallback default.
- `service`: `ServiceSpec` builder, middleware mock re-exports
  (`EvmMiddleware`, `MockEvmServiceManager`, `AvsOperator`),
  `InProcRunner` (real WASM via `wavs-engine`),
  `MockHandler` (`SimpleServiceManager` + `SimpleSubmit` deployable pair
  for on-chain envelope validation), `SubprocessRunner`
  (functional spawn + health probe + service registration, feature
  `subprocess`).
- `lifecycle`: `manual_input_json` / `manual_input_raw`, `wait_for` /
  `wait_until` polling helpers, `assert_within` tolerance check.
- `envelope`: canonical `Envelope` / `SignatureData` shape mirroring
  `@wavs/solidity@0.6.x`, `sign_envelope`, `submit_envelope` (on-chain
  `handleSignedEnvelope` dispatch), `sort_signature_data`,
  `event_id_from_nonce` / `event_id_from_seed`,
  `Envelope::message_hash` / `signing_hash`.
- `harness::TestHarness<P>` convenience wrapper.

## What's not in v1

- Full trigger-emission + quorum-waiting on the subprocess tier (the
  spawn + service-registration loop ships; the rest is composable today
  by combining `SubprocessRunner` with the harness's `chain::*`
  primitives — bundled helpers land in the next PR).
- Pyth + other oracle shapes (Chainlink AggregatorV3 ships; the same
  `anvil_set_code` pattern applies, only the bytecode differs).
- Cosmos fork support.
- A `with_deploy(closure)` builder hook on `TestHarness` (deferred until
  async-closure ergonomics stabilize; compose primitives directly today).
- Operator-side oracle mocking — needed before downstream apps can run
  their real strategy WASM end-to-end via the InProc runner.
- A version of the harness compatible with newer alloy lines used by
  downstream apps — see "Cross-repo alloy version barrier" above.
