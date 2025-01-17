# WAVS

![Banner!](docs/images/wavs.png)

[![Project Status: Active -- The project has reached a stable, usable
state and is being actively
developed.](https://img.shields.io/badge/repo%20status-Active-green.svg?style=flat-square)](https://www.repostatus.org/#active)

WAVS is a next-generation AVS (Actively Validated Service) platform, making it easy to create, manage, and operate high-performance AVSs. By providing a base layer of AVS infrastructure, WAVS allows builders to focus solely on implementing the core logic of their services, which are written in Rust (with support for more languages coming soon), compiled to WASM, and deployed as lightweight service components. Triggered by onchain events, these components are run offchain by operators in the WAVS WASI runtime at near-native speed, and the results are brought verifiably onchain. A service of services, WAVS allows an AVS to dynamically run and manage multiple components that work together to build flexible and intelligent applications.

## Multichain-ready

WAVS is designed for the multichain, enabling services to be triggered and results to be posted across different chains. Initially supported on Ethereum and Cosmos, WAVS creates a cross-chain computational layer that is decentralized and verifiable.

## Guides

- [Quickstart](docs/QUICKSTART.md)
- [Docs](docs/README.md)

## Run locally

```bash
# copy env files
cp packages/aggregator/.env.example packages/aggregator/.env
cp packages/cli/.env.example packages/cli/.env
cp packages/wavs/.env.example packages/wavs/.env

# brew install just
# - if you get 'ERROR [internal] load metadata ...': https://stackoverflow.com/a/71665244
just docker-build

# MacOS Docker:
# Docker Engine -> Settings -> Resources -> Network -> 'Enable Host Networking'
# or
# brew install chipmk/tap/docker-mac-net-connect && sudo brew services start chipmk/tap/docker-mac-net-connect
docker compose up

# Interact
docker exec -it wavs bash
wavs-cli deploy-service --component /wavs/components/eth_trigger_echo.wasm
wavs-cli add-task --input "echo value" --service-id <SERVICE_ID>
```

## Overview

WAVS is node software for operators that runs an AVS [WASI](https://wasi.dev/) runtime which is easily configurable and can serve multiple AVSs. The logic for each AVS is deployed as a WASI service component. These service components are sandboxed from each other and from the node's operating system. This way, operators and AVS services maintain a clean separation, with AVS builders uploading components and operators having to opt-in to each service.


![WAVS overview](./docs/images/flow.png)

Onchain events can trigger a service's WASI components, which are run offchain by operators in the AVS WASI runtime. Results are then verified and brought back onchain, enabling the decentralized execution of offchain services which are verifiable onchain.

1. A service is defined, including a WASI component and triggers.
2. Service operators listen for onchain events.
3. An onchain event triggers a task, and operators run the WASI component offchain.
4. Operators sign the result of the task.
5. (Optional for EVM) The signatures are sent to the aggregator contract which consolidates the results into one transaction.
6. The result is submitted onchain.

