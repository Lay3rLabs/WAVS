# WAVS (WIP)

![Banner!](docs/images/wavs.png)

[![Project Status: Active -- The project has reached a stable, usable
state and is being actively
developed.](https://img.shields.io/badge/repo%20status-Active-green.svg?style=flat-square)](https://www.repostatus.org/#active)


WAVS is a platform for building AVSs, allowing for decentralized execution of offchain computations with results that can be verified on the blockchain. A service of services, WAVS allows an AVS to run and dynamically dynamically run and manage multiple services (compiled WASM/WASI components) that work together to build flexible and intelligent applications. 

![WAVS overview](./docs/images/flow.png)



## Guides

- [Quickstart](./docs/QUICKSTART.md)
- [Develop a Service](./docs/AUTHORING_COMPONENTS.md)
- [Manually deploy a task](./docs/MANUAL_TASK.md)


This is an AVS operator node, that is quickly configurable to easily serve
logic for many AVSs, each one sandboxed from each other and the node's
operating system.

To achieve this sandboxing, we use WASI components, with limited access to system resources.

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

