
# WAVS Architecture

WAVS is a highly configurable AVS operator node. It allows you to easily run logic for multiple AVSs, with each service securely sandboxed from others and from the node's operating system.

## Sandboxing with WASI

WAVS uses [WASI](https://wasi.dev/interfaces) components to achieve strong isolation. Each AVS runs as a WASI component, with restricted access to system resources for security and reliability.

## Who Runs a WAVS Node?

WAVS nodes are operated by "Operators." These participants are similar to validators in proof-of-stake blockchains:

- Operators receive voting power through delegation.
- Their nodes perform off-chain actions, which are then submitted and verified on-chain.

To serve an AVS, an operator must explicitly opt-in. This is done via an HTTP endpoint on the node, where the operator provides the address and chain of the ServiceManager contract.

## Separation of Concerns

The AVS team is responsible for:
- Coding all WASI components
- Deploying AVS contracts on-chain

Operators and AVS teams are completely independent. AVS teams do not have access to operator systems, ensuring a clean separation of concerns.

## Internal Subsystems and Flow

At a high level, we have these subsystems:

1. Trigger Manager: Parses events and filters for registered services
2. Engine: Executes the WASM components
3. Submission: Submits results to the aggregator (a completely separate node)

These each run in their own thread and are orchestrated by the Dispatcher, typically over crossbeam unbounded channels

Administration of a node is done through a HTTP server, which holds an instance of the dispatcher and can call it directly

As a rule of thumb, we do not block - async tasks are spawned onto the tokio runtime, and we do not crash - errors are logged via the tracing mechanism and inspected via tools like Jaeger