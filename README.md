# Wavs

This is an AVS operator node, that is quickly configurable to easily serve
logic for many AVSes, each one sandboxed from each other and the node's
operating system.

To acheive this sandboxing, we use WASI components, with limited access to system resources.

## Running

This repo is organized with multiple packages. The default binary when running from the workspace is `wavs`, 
but it will only find the config file automatically when running from within the package.

To run from the root workspace (with `localhost` chain):

```
cargo run -- --home=./packages/wavs --chain=localhost
```

Similarly, it will pick up the `.env` from the current working directory.

## Persona

This node should be run by an "Operator". This is very much like a "validator" on a PoS chain.
It receives commitments (stake) on the chain, which provide it with voting power, and it performs
some off-chain actions, which are submitted on-chain and verified.

Currently, for demo purposes, we expose the HTTP API to make adding new WASI components easy.
However, in any realistic scenario, each node is managed by an operator, which must explicitly
opt-in to running the AVS software, and then register their intention on-chain, in order
to collect commitments.

The AVS team, which will code all the WASI components, and deploy (and write?) the AVS contracts
on-chain should be completely independent of the operators for a clean separation of concerns,
and thus have no access to their system.

## Architecture

Start by looking at this overview diagram of the various components of the system

![Architecture Overview](./docs/images/ArchOverview.svg)