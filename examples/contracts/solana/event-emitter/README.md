# Solana fixture: `event-emitter`

Minimal Anchor program used by the WAVS Solana (v1, trigger-only) demo and
by `packages/layer-tests/tests/solana_e2e.rs`. One instruction (`emit`)
takes a payload and emits a `MessageEmitted` Anchor event:

```text
Program <id> invoke [1]
Program log: Instruction: Emit
Program data: <base64-discriminator + borsh-payload>
Program <id> success
```

The `Program data:` line is what the WAVS Solana trigger stream's
`SolanaEventFilter::Discriminator` filter selects on, using the
8-byte Anchor event discriminator `sha256("event:MessageEmitted")[..8]`
(value `0xab 0x0f 0xdc 0xb7 0x2d 0x7f 0xb7 0x27`).

## Demo flow (Solana event → WAVS → EVM `SimpleSubmit`)

Three terminals:

```sh
# 1. Local Solana validator (RPC 8899, WS 8900)
just start-solana-validator

# 2. Local EVM testnet (RPC 8545)
just start-anvil

# 3. Build + deploy the fixture program
just deploy-solana-fixture
```

Then start WAVS configured with a service.json that has:

- A `solana:devnet` chain entry pointing at `http://127.0.0.1:8899` /
  `ws://127.0.0.1:8900`
- A `SolanaProgramEvent` trigger using the program id printed by
  `anchor deploy` and `SolanaEventFilter::Discriminator` with the
  `MessageEmitted` discriminator above
- The `solana-event-relay` operator component
  (`examples/build/components/solana_event_relay.wasm`)
- An `Aggregator` submit pointing at a deployed `SimpleSubmit` service
  handler on anvil

Fire an `emit` instruction (any signer + a `Vec<u8>` payload). The
relay strips the discriminator + borsh length prefix, encodes a
`DataWithId { triggerId: slot, data: payload }`, and the aggregator
relays it to the EVM `SimpleSubmit` contract on anvil.

## Toolchain

- Anchor `0.32.1` (matches `contracts/svm-middleware/`)
- Solana CLI from the [Anza
  installer](https://docs.anza.xyz/cli/install-solana-cli-tools): `sh
  -c "$(curl -sSfL https://release.anza.xyz/stable/install)"`

The justfile recipes check for both and print install hints if either
is missing — we do not auto-install.

## What this is NOT

- Not a v2 middleware program. The fixture has no operator registry,
  signature verification, or submission path. That is `contracts/svm-middleware/`
  in the parent workspace, and the design doc's v2 sketch tracks it.
- Not a production event shape. The single `Vec<u8>` payload is
  deliberately minimal so the demo focuses on the trigger-stream and
  WIT-bindings story. v2 will have realistic event shapes once the
  submission path actually targets Solana.
