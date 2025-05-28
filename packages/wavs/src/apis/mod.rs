// This is currently a scratchpad to define some interfaces for the system level.
// It probably should be pulled into multiple files before merging, but I think easier to visualize and review all together first.

pub mod dispatcher;
pub mod engine;
pub mod submission;

/***
 *
 * High-level system design
 *
 * The main component is the Dispatcher, which can receive "management" calls via the http server
 * to determine its configuration. It works at the level of "Services" which are independent
 * collections of code and triggers that serve one AVS.
 *
 * Principally the Dispatcher manages workflows by the following system:
 *
 * * When the workflow is created, it adds all relevant triggers to the TriggerManager
 * * It continually listens to new results from the TriggerManager, and executes them on the WasmEngine.
 * * When the WasmEngine has produced the result, it submits it to the verifier contract.
 *
 * The TriggerManager has it's own internal runtime and is meant to be able to handle a large number of
 * async network requests. These may be polling or event-driven (websockets), but there are expected to be quite
 * a few network calls and relatively little computation.
 *
 * The WasmEngine stores a large number of wasm components, indexed by their digest.
 * It should be able to quickly execute any of them, via a number of predefined wit component interfaces.
 * We do want to limit the number of wasmtime instances at once. For testing, a simple Mutex around the WasmEngine
 * should demo this. For real usage, we should use some internal threadpool like rayon at set a max number of
 * engines running at once. We may want to make this an async interface?
 *
 * Once the results are calculated, they need to be signed and submitted to the chain (or later to the aggregator).
 * We can do this in the operatator itself, or design a new subsystem for that. (Open to suggestions).
 *
 * I think the biggest question in my head is how to handle all these different runtimes and sync/async assumptions.
 * * Tokio channels is one way (which triggers use as it really matches this fan-in element well) - which allow each side to be either sync or async.
 * * Async code can call sync via `tokio::spawn_blocking`, but we may need some limit on how many such threads can be active at once
 *
 * Currently, I have a strong inclination to use sync code for:
 * * WasmEngine (it seems more stable)
 * * ReDB / KVStore (official recommendation is to wrap it with `tokio::block_in_place` or such if you need it async)
 *
 * And use async code for:
 * * TriggerManager
 * * HTTP Server
 *
 * I think the internal operation of the Dispatcher is my biggest question.
 * Along with how to organize the submission of results.
 * And then how to somehow throttle concurrent access to the WasmEngine.
 *
 ***/

/*

General execution workflow:

<Triggers> --Action--> <WasmEngine> --Result--> <Submission>

           mpsc channel               mpsc channel

Implementation: Actual pipeline is orchestrated by Dispatcher.
"Dispatcher" is like "event dispatcher" but also stores state and can reconstruct the other ones
Dispatcher should be quick, it has high-level system overview, just needs to delegate work to subsystems.


Idea 1

<Triggers> --TriggerAction-->     Dispatcher        --ChainMessage-->  <Submission>
                        - call WasmEngine
                        (call/response interface)


Idea 2

<Triggers> --TriggerAction-->  Dispatcher  --WasmRequest--> WasmEngine --WasmResult--> Dispatcher --ChainMessage-->  <Submission>
  async       (buffer?)      sync (select)                                         sync (select)

Trigger Action:
- (service, workflow) id
- task id (from queue)
- payload data

WasmRequest:
- (service, workflow id)
- task id
- payload data
- wasm digest

WasmResult:
- (service, workflow id)
- task id
- wasm result data

ChainMessage:
- (service, workflow id) ?? Do we need this anymore?
- task id
- wasm result data
- submit (hd_index, verifier_addr)

Dispatcher Thread 1 and 2 maintain some mapping by querying the workflow for the next step to execute.

HD Index must not be shared between different services.
For now assume all Submit in one service use the same HD Index.

Notes:

Dispatcher should allow multiple trigger actions to be run at the same time (some limit).

- WasmEngine can manage internal threadpool / concurrency limits
- Dispatcher has channel to WasmEngine, sends onshot channel with request to get result

* Look at backpressure
* Tracing, logging, metrics are important to monitor this pipeline

*/

/*

General management workflow
Sync calls on Dispatcher.

On load:
- Dispatcher loads all current state (list of registered services - workflows + triggers)
- Triggers wasm to refresh state if needed??
- Initializes all channels and subsystems (trigger, wasm engine, submission)
- Adds all triggers to trigger manager

On HTTP Request (local, from authorized agent):
- Update Dispatcher state
  - May store new wasm -> wasm engine (internal persistence)
  - May add/update triggers in trigger subsystem
  - Stores new services locally to manage workflows when triggers send actions

Management interface of Dispatcher may be somewhat slow, unlike the execution pipeline.
We also don't expect high-throughput here and could even limit to one management
operation at a time to simplify code for now.

HTTP server should call in `spawn_blocking` to avoid blocking the async runtime.
We can even use a mutex internally to ensure only one management call processed at a time.

Idea: HTTP server is outside of the Dispatcher and contains it as state once the Dispatcher
is properly initialized. It can then call into the Dispatcher to adjust running services.

- Management - set up workflows, add components
- Execution - run workflows, triggers -> wasm -> submit

*/

/*
Thoughts:

- Testability
  - do we need traits for some of these objects?
  - do we fake at the level of channels?
  - where do we allow different configurations?
  - focus on testing components of the system without needing a full chain
  - mocking out wasm engine to run some closure not full wasmtime
- Traceability
  - how do we log and trace the execution of a workflow?
  - how do we monitor the system?
  - important for end-to-end testing and real production

*/
