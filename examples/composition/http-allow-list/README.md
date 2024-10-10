# Static Access Control for Outgoing HTTP Allowed Hosts

## Setup

If you have everything previously setup, you may just need to install `wac` CLI:

```bash
cargo install wac-cli
```

Otherwise, if you setting up for the first time, install `wac-cli`, `cargo-component`, and `wkg`:

```bash
cargo install wac-cli wkg cargo-component
```

Also, if you haven't install the WASI Preview 1 target:

```bash
rustup target add wasm32-wasip1
```

Set the default registry configuration:
```bash
wkg config --default-registry wa.dev
```

Also, you will need the latest `avs-toolkit-cli`. `git clone` this [repo](https://github.com/Lay3rLabs/avs-toolkit) and then from within the repo dir:
```bash
cargo install --path tools/cli
```


## Testing the Original Oracle Example

First, let's test that the Oracle example runs without any of our access control composition.

You can either build the [Simple AVS Oracle example](https://github.com/Lay3rLabs/example-avs-oracle/tree/main/wasi/oracle-example)
yourself, or we can just download the published version.

```bash
wkg get lay3r-examples:oracle-example
```

This downloads the latest version of the `oracle-example` in the current directory as `lay3r-examples_oracle-example@0.1.0.wasm`.

Let's run locally without any access control composition and a CoinGecko API key:
```bash
avs-toolkit-cli wasmatic run \
    --wasm-source lay3r-examples_oracle-example@0.1.0.wasm  \
    --envs "API_KEY=CG-PsTvxDqXZP3RD4TWNxPFamcW"
```

You should see a recent BTCUSD price printed to the terminal like:
```bash
{"price":"60035.06"}
```

## Testing Deny Unallowed Host

```bash
cd http-allowed-coingecko
```

Then open up `src/lib.rs` in your editor you should see L8:
```rust
const ALLOWED: [&str; 1] = ["api.coingecko.com"];
```

Temporarily, let's change to only allow outgoing host to "layer.xyz" so that our `oracle-example` will
be denied the outgoing HTTP request to CoinGecko's API:

```rust
const ALLOWED: [&str; 1] = ["layer.xyz"];
```

And let's compile a release build:

```bash
cargo component build --release
```

Now, let's compose this access control component with the `oracle-example` that you dowloaded:

```bash
wac plug  \
  --plug ../../../../target/wasm32-wasip1/release/http_allowed_coingecko.wasm \
  lay3r-examples_oracle-example@0.1.0.wasm \
  -o should-deny.wasm
```

You can also, swap the local file paths for registry published package `lay3r-examples_oracle-example@0.1.0.wasm`
could be `lay3r-examples:oracle-example`.

Let's run the component locally:
```bash
avs-toolkit-cli wasmatic run \
    --wasm-source should-deny.wasm  \
    --envs "API_KEY=CG-PsTvxDqXZP3RD4TWNxPFamcW"
```

You should see that the outgoing request was denied:
```bash
Error: failed to send request
```

## Testing Only Allowing CoinGecko API

Let's open up `src/lib.rs` in your editor change L8 back to:
```rust
const ALLOWED: [&str; 1] = ["api.coingecko.com"];
```

And let's compile a release build:

```bash
cargo component build --release
```

Now, let's compose this access control component with the `oracle-example` that you dowloaded:

```bash
wac plug  \
  --plug ../../../../target/wasm32-wasip1/release/http_allowed_coingecko.wasm \
  lay3r-examples_oracle-example@0.1.0.wasm \
  -o should-allow.wasm
```

Let's run the component locally:
```bash
avs-toolkit-cli wasmatic run \
    --wasm-source should-allow.wasm  \
    --envs "API_KEY=CG-PsTvxDqXZP3RD4TWNxPFamcW"
```

You should see a recent BTCUSD price printed to the terminal like:
```bash
{"price":"59041.258"}
```

Since both the components that we want to compose are actually published to the registry,
you can also do:

```bash
wac plug  \
  --plug lay3r-examples:http-allowed-coingecko \
  lay3r-examples:oracle-example \
  -o from-registry-should-allow.wasm
```

Let's run the component locally:
```bash
avs-toolkit-cli wasmatic run \
    --wasm-source from-registry-should-allow.wasm  \
    --envs "API_KEY=CG-PsTvxDqXZP3RD4TWNxPFamcW"
```

You should see a recent BTCUSD price printed to the terminal like:
```bash
{"price":"58955.1"}
```
