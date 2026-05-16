# wavs-test-harness

Reusable integration test harness for WAVS apps.

Tracks issue [Lay3rLabs/WAVS#1147](https://github.com/Lay3rLabs/WAVS/issues/1147).

## Status

Scaffold. Only the module layout exists today. See `src/lib.rs` for the planned
surface area. Each step in the plan delivers one module.

## What this crate is for

Downstream app repos — for example `wavs-defi`, `wavs-aave-guardian`,
`wavs-prediction-market` — write integration tests that drive the full WAVS path:

```
trigger -> operator component -> aggregator (sig + quorum) -> service handler -> app contract
```

`wavs-test-harness` provides the chain control, service-lifecycle, and assertion
primitives so each downstream test only has to specify its app-specific
deployment and assertions.

## Tier matrix (planned)

| Tier | Stages run | Stages mocked | Use case |
|---|---|---|---|
| **Logic** | trigger + contract calls | compute/aggregate/submit | unit-level vault tests with mock signatures |
| **InProc** (default) | compute (real WASM) + aggregate (in-memory quorum) + submit | dispatcher internals | deterministic PR tier on local Anvil |
| **Subprocess** (preview) | everything in the real `wavs` binary | nothing | nightly fork + pre-release |

## Tier selection

Add `wavs-test-harness` as a `[dev-dependencies]` entry only, never as a regular
dependency. This keeps `utils/test-utils` out of production builds.

```toml
[dev-dependencies]
wavs-test-harness = { git = "https://github.com/Lay3rLabs/WAVS", rev = "<commit-sha>" }
```

Branch references are not supported in CI — pin to a commit SHA.

## Layer-tests relationship

`packages/layer-tests` continues to run as-is. Once this harness stabilizes,
`layer-tests` is expected to migrate onto it. Treat this crate as canonical for
new integration tests.
