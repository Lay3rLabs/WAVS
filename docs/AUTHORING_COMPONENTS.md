# Develop Wasm components

## Prerequisites

If you haven't already, install the Rust toolchain with at least version `1.80.0`,
[see instructions](https://www.rust-lang.org/tools/install).

Even though we will be building a Wasm component that targets WASI Preview 2, the Rust
`wasm32-wasip2` build target is not quite ready yet. So we will use
[`cargo-component`](https://github.com/bytecodealliance/cargo-component) to compile
`wasm32-wasip1` binaries and package to use WASI Preview 2.

If haven't yet, add the WASI Preview 1 target:
```bash
rustup target add wasm32-wasip1
```

Install `cargo-component` and `wkg` CLIs:
```bash
cargo install cargo-component wkg
```

Set default registry configuration, where the [`lay3r:avs`](https://wa.dev/lay3r:avs)
WIT package is published:
```bash
wkg config --default-registry wa.dev
```
For more information about configuration, see
the [wkg docs](https://github.com/bytecodealliance/wasm-pkg-tools).


## Setup a New Project

To create an new Wasm component for use with Layer's Wavs, let's use `cargo component`
to scaffold a new project with a task queue trigger. Feel free to choose a different project
name then "my-task".

```bash
cargo component new --lib --target lay3r:avs/task-queue my-task && cd my-task
```

Let's do the first build to generate the `src/bindings.rs` file. Afterwards, you can do the
familiar `cargo` commands such as `cargo test` and `cargo clippy`. It may be helpful to inspect
`src/bindings.rs` during development to see the type information for producing the Wasm component.

**IMPORTANT**: System environment variables must be prefixed with `WAVS_ENV_` (e.g., `WAVS_ENV_COINGECKO_API_KEY`). Environment variables that do not contain this prefix will not be accessible by the component.

```bash
cargo component build
```

It is helpful to add the `layer-wasi` crate to make outgoing HTTP requests easily. See the
[oracle example](https://github.com/Lay3rLabs/example-avs-oracle/tree/main/wasi/oracle-example).


```bash
cargo add --git https://github.com/Lay3rLabs/example-avs-oracle layer-wasi
```

Then start developing your application. Start by editing the `src/lib.rs` file. Also, see
the [oracle example](https://github.com/Lay3rLabs/example-avs-oracle/tree/main/wasi/oracle-example)
and [square example](https://github.com/Lay3rLabs/avs-toolkit/tree/main/wasi/square).


## Unit Testing

For running unit tests, the familiar commands will work.

```bash
cargo test
```

## Deploying

First, let's do a release build of the component:

```bash
cargo component build --release
```

Upload the compiled Wasm component to the Wavs node.
```bash
curl -X POST --data-binary @./target/wasm32-wasip1/release/my_task.wasm http://localhost:8081/upload
```

Copy the digest SHA returned.
Choose a unique application name string and use in the placeholder below `curl` commands.

```bash
read -d '' BODY << "EOF"
{
  "name": "{PLACEHOLDER-UNIQUE-NAME}",
  "digest": "sha256:{DIGEST}",
  "trigger": {
    "queue": {
      "taskQueueAddr": "{TASK-QUEUE-ADDR}",
      "hdIndex": 0,
      "pollInterval": 5
    }
  },
  "permissions": {},
  "testable": true
}
EOF

curl -X POST -H "Content-Type: application/json" http://localhost:8081/app -d "$BODY"
```

## Testing Deployment

To test the deployed application on the Wavs node, you can provide `input` test data
that your application expects. The server responds with the output of the applicaton without
sending the result to the chain.

```bash
curl --request POST \
  --url http://localhost:8081/test \
  --header 'Content-Type: application/json' \
  --data '{
  "name": "{PLACEHOLDER-UNIQUE-NAME}",
  "input": {}
}'
```
