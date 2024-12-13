# WAVS

![Banner!](docs/images/wavs.png)

[![Project Status: Active -- The project has reached a stable, usable
state and is being actively
developed.](https://img.shields.io/badge/repo%20status-Active-green.svg?style=flat-square)](https://www.repostatus.org/#active)

WAVS is a next-generation AVS platform, making it easy to create, manage, and operate high-performance AVSs. By providing a base layer of AVS infrastructure, WAVS allows builders to focus solely on implementing the core logic of their services, which are written in Rust and deployed as lightweight WASI components. These components are run offchain by operators in the WAVS WASI runtime at near-native speed, and the results are brought verifiably onchain. A service of services, WAVS allows an AVS to dynamically run and manage multiple components that work together to build flexible and intelligent applications.

## Guides

- [Quickstart](docs/QUICKSTART.md)
- [Create a Service](docs/AUTHORING_COMPONENTS.md)
- [Docs](docs/README.md)

## Overview

WAVS is a node software for operators that runs an AVS [WASI](https://wasi.dev/) runtime which is easily configurable and can serve multiple AVSs. The logic for each AVS is deployed as a WASI component. These WASI components have limited access to system resources and are sandboxed from each other and from the node's operating system. This way, operators and AVS services maintain a clean separation, with AVS builders uploading components and operators having to opt-in to each service.


![WAVS overview](./docs/images/flow.png)

Onchain events can trigger a service's WASI components, which are run by operators in the AVS WASI runtime. Results are then verified and brought back onchain, enabling the decentralized execution of offchain services which are verifiable onchain.

1. An AVS uploads their WASI component containing their service logic.
2. Operators listen for onchain events.
3. An onchain event triggers a task, and operators run the WASI component offchain.
4. Operators sign the result of the task, and the signatures are sent to the aggregator contract.
5. Signatures are aggregated, and the result is sent onchain.

