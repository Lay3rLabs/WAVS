# Setting up Jaeger for tracing in tests

A quick guide to setting up Jaeger for collecting traces from Rust tests using the OpenTelemetry Protocol (OTLP).


## Prerequisites

 - ensure Docker is installed on your system.

## Set up Jaeger

### 1. Start Jaeger Using Docker

Run the following command in a separate command line to start a Jaeger instance:

```bash
docker run \
  --name jaeger \
  -p 4317:4317 \
  -p 16686:16686 \
  jaegertracing/jaeger:2.5.0
```

- ports:
  - `4317`: OTLP gRPC endpoint for receiving traces.
  - `16686`: Jaeger UI for visualizing traces.

### 2. Enable Jaeger endpoint

Update the configuration file `packages/layer-tests/layer-tests.toml` and uncomment the line:
```bash
jaeger = "http://localhost:4317"
```

### 3. Run your tests

Run your tests as usual:
```bash
cd packages/layer-tests && cargo test
```
- the OpenTelemetry tracer will send traces to the Jaeger server at `http://localhost:4317`.
- if everything is correct, traces generated during the tests will be collected by Jaeger at shutdown.

### 4. View traces in Jaeger UI

Open the Jaeger UI in your browser:
```
http://localhost:16686
```
- select the service name `wavs-tests` from the dropdown
- search for traces and inspect them, for example, the `execute` trace is what happens when a trigger gets executed by the engine.


### For production usage

- Setup Jaeger to use a persistent storage backend (e.g., Elasticsearch, Cassandra, etc.) instead of the default in-memory storage.
- The service names will be `wavs` and `wavs-aggregator`
