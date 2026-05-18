# `solana-event-relay`

WAVS operator component for the v1 SVM trigger demo. Receives a
`TriggerData::SolanaProgramEvent` from the Solana trigger stream, strips
the Anchor `MessageEmitted` framing
(`sha256("event:MessageEmitted")[..8]` discriminator + borsh `Vec<u8>`
length prefix), and emits a `DataWithId`-encoded `WasmResponse` for the
existing `SimpleSubmit` EVM service handler.

`triggerId` is the Solana slot — keeps the EVM `DataWithId` monotonic
per-slot and disambiguates distinct events without requiring the relay
to allocate its own ids. The full `(slot, signature, instruction_index,
inner_instruction_index, log_index)` replay-protection tuple is
enforced upstream in the dispatcher (see
`packages/wavs/src/subsystems/trigger.rs::SolanaReplayCache`).

See `examples/contracts/solana/event-emitter/README.md` for the
end-to-end demo flow.
