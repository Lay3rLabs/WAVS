# E2E Tests

This is the typical first stop for debugging and developing WAVS.

The flow is usually editing `packages/layer-tests/layer-tests.toml` to isolate on a specific test, like:

```toml
mode = {"isolated" = [
  {evm = "timer_aggregator"},
]}
```

Then running:

```bash
cd packages/layer-tests && cargo test
```

# Live telemetry

Sometimes it helps to run a live instance of WAVS and look at Jaeger or Prometheus metrics

### Start the backend

`just start-dev`

This will start wavs, jaeger, and prometheus with a new temp directory for wavs data

You'll need to open another terminal to interact with it

### Run dev-tools

You can run `just dev-tool help` to see available commands in the dev tool, and then `just dev-tool *` to run the command. For example:

1. `just dev-tool deploy-service`
2. `just dev-tool send-triggers --count 10`

### View telemetry

- Jaeger UI: [http://localhost:16686](http://localhost:16686)
- Prometheus UI: [http://localhost:9090](http://localhost:9090)

See [TELEMETRY.md](TELEMETRY.md) for more details on telemetry setup and usage.
