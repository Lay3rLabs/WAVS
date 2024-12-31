## Start local chain

```bash
starship start --config ./starship.yaml
```

## (re)build component
in repo root:

```bash
just wasi-build
```

## Run via wavs test
in repo `packages/wavs`:

```bash
RUST_LOG="info,wavs=debug" cargo test --features e2e_tests_ethereum_cosmos_query e2e_tests -- --nocapture
```

## Stop local chain

```bash
starship stop --config ./starship.yaml
```