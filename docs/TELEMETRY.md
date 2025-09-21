# Table of contents

1. [Jaeger for tracing](#jaeger-for-tracing)
2. [Prometheus for metrics](#prometheus-for-metrics)

# Setting up Jaeger for tracing in tests <a id="jaeger-for-tracing"></a>

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

---

# Setting up Prometheus for collecting metrics in tests <a id="prometheus-for-metrics"></a>

How to run Prometheus and configure wavs to collect metrics during Rust tests using the OpenTelemetry Protocol (OTLP).

## Prerequisites

 - ensure Docker is installed on your system.

## Set up Prometheus

### 1. Configure a `prometheus.yml` file 

In order to run a Prometheus instance in Docker, we need to have a `prometheus.yml` configuration file. For this test, an empty file is sufficient. If you are in the main `wavs` directory, there is already one in the `config/` directory.

### 2. Start Prometheus Using Docker

Run the following command in a separate command line to run a Prometheus instance:

```bash
docker run \
  --name prometheus \
  -p 9090:9090 \
  -v ./config/prometheus.yml:/etc/prometheus/prometheus.yml \
  prom/prometheus \
  --config.file=/etc/prometheus/prometheus.yml \
  --web.enable-otlp-receiver
```

- ports:
  - `9090`: Prometheus UI and OTLP receiver endpoint for receiving metrics.
- configuration:
  - `-v ./config/prometheus.yml:/etc/prometheus/prometheus.yml`: mounts your local `prometheus.yml` file into the container, allowing you to configure Prometheus externally.
  - `--config.file=/etc/prometheus/prometheus.yml`: specifies the path to the configuration file inside the container.
  - `--web.enable-otlp-receiver`: enables Prometheus to accept metrics via the OTLP protocol.

### 3. Enable Prometheus collection endpoint

Update the configuration file `packages/layer-tests/layer-tests.toml` and uncomment the line:
```bash
prometheus = "http://localhost:9090"
```

### 4. Run your tests

Run your tests as usual:
```bash
cd packages/layer-tests && cargo test
```
- the OpenTelemetry metrics will be periodically uploaded to Prometheus server at `http://localhost:9090`.

### 5. View metrics in Prometheus

Open Prometheus in your browser:
```
http://localhost:9090
```
- in the field `Enter expression` paste: `{__name__=~".+"}` and press Execute- it will show all of the metrics collected so far.
- auto-completion works really well, you can put just the name of the whole counter to see its data: `http_registered_services` for example
- try using tabs `Table` and `Graph` to see different data representations.
- official Prometheus query examples: https://prometheus.io/docs/prometheus/latest/querying/examples/