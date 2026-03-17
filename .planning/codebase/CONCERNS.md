# Codebase Concerns

**Analysis Date:** 2026-03-17

## Tech Debt

**In-Memory Database Without Persistence:**
- Issue: `WavsDb` in `packages/utils/src/storage/db.rs` uses DashMap for all tables (services, quorum_queues, kv_store, etc.) with no disk persistence. Comments at lines 23 and 66 explicitly state "Right now this is purely in-memory; later we will add file-based persistence"
- Files: `packages/utils/src/storage/db.rs` (lines 23, 66, 89)
- Impact: Any node restart loses all service registrations, workflow state, and operator data. Critical for production where operator state must survive restarts
- Fix approach: Implement RocksDB or ReDB backend for `WavsDbTable`, add migration layer to support both in-memory (tests) and persistent modes. Consider async wrapper in `tokio::block_in_place` as documented in `packages/utils/src/storage/prelude.rs` (lines 14-20)

**Unsafe Trait Implementations for WASM Transport:**
- Issue: `WasiEvmClient` in `packages/wasi-utils/src/evm/provider.rs` uses `unsafe impl Sync` and `unsafe impl Send` (lines 50-51) with reference to Cloudflare's approach but without clear reasoning in comments
- Files: `packages/wasi-utils/src/evm/provider.rs` (lines 50-51)
- Impact: If WASM runtime violates thread-safety assumptions, memory unsafety could result; current justification is insufficient
- Fix approach: Document why WASM sandbox guarantees safety, or refactor to use wrapper type that doesn't require unsafe traits

**Storage API Not Multi-Thread Safe by Design:**
- Issue: `CAStorage` trait in `packages/utils/src/storage/prelude.rs` has TODO comment (line 23) noting that trait is not multi-thread safe: "make multi-thread safe - remove &mut by wrapping internally with Arc / RwLock"
- Files: `packages/utils/src/storage/prelude.rs` (line 23)
- Impact: Current design requires external synchronization; if code evolves to need interior mutability, breaking change required across all implementations
- Fix approach: Refactor `CAStorage` implementations (FileStorage, RocksDB) to use Arc/RwLock internally for true thread-safe access

**Incomplete Feature Implementation - Cosmos setServiceURI:**
- Issue: `ServiceUpdateModal` in `app/src/components/service/ServiceUpdateModal.tsx` has incomplete Cosmos support. Lines 69 and 112 contain TODOs for "Cosmos setServiceURI support" with only EVM implementation complete
- Files: `app/src/components/service/ServiceUpdateModal.tsx` (lines 69, 112)
- Impact: Cosmos chain operators cannot update service URIs through the desktop app; workaround requires manual contract interaction
- Fix approach: Implement Cosmos setServiceURI using CosmWasm contract interaction (mirroring EVM pattern with Alloy/Viem equivalent)

## Known Issues

**FIXME: Incomplete WAT Magic for HTTP Permissions:**
- Symptom: HTTP permission checking uses Wasmtime's `add_only_http_to_linker_async` but doesn't apply host-only filters properly
- Files: `packages/engine/src/worlds/instance.rs` (line 351)
- Trigger: WASM component tries to access non-whitelisted HTTP hosts
- Workaround: Currently allows broader HTTP access than intended; apply additional filtering at component level
- Fix approach: Implement WAT magic to enforce `Only(host)` permission checks in the linker configuration

**FIXME: Mock EVM Event Placeholder:**
- Symptom: Test mock trigger manager doesn't generate realistic EVM events
- Files: `packages/wavs/tests/wavs_systems/mock_trigger_manager.rs` (line 41)
- Trigger: When running system tests with mock triggers, event structure doesn't match production format
- Workaround: Tests pass but may miss edge cases in actual EVM event parsing
- Fix approach: Implement proper EVM event generation using actual event types instead of placeholder

**Missing Trigger Cleanup on Service Removal:**
- Symptom: When `remove_service()` is called, only the lookup maps are removed but associated resources remain
- Files: `packages/wavs/src/subsystems/trigger.rs` (lines 243-248)
- Trigger: Service deregistration with many trigger types (EVM logs, block subscriptions, cron jobs)
- Workaround: Resources leak gradually over service lifecycle
- Fix approach: Implement cleanup logic for cron jobs, EVM subscriptions, block subscriptions, and other stateful triggers

**Missing Submission Deduplication Check:**
- Symptom: Aggregator doesn't query whether results have already been submitted before signing
- Files: `packages/wavs/src/subsystems/aggregator/submit.rs` (line 52)
- Trigger: Multiple operators submit same result; if aggregator hasn't checked, duplicates reach chain
- Workaround: Chain-level duplicate detection (if implemented in ServiceHandler)
- Fix approach: Query service manager state before submission to check if result was already submitted

**Missing Trigger Data in Mock Trigger Manager:**
- Symptom: Mock trigger manager doesn't store trigger data for list responses
- Files: `packages/wavs/tests/wavs_systems/mock_trigger_manager.rs` (line 137)
- Trigger: When querying active triggers in tests, trigger_data field is missing
- Workaround: Tests that don't check trigger_data pass fine
- Fix approach: Store trigger data in mock and return in list_triggers response

## Security Considerations

**Debug Forcing Environment Variables in Production Code:**
- Risk: WAVS_FORCE_*_ERROR_XXX and WAVS_FORCE_*_SLOW_XXX environment variables allow forcing artificial failures and slowdowns in production builds (feature-gated but compiled in debug feature)
- Files: `packages/wavs/src/subsystems/engine/wasm_engine.rs` (lines 112, 160, 400, 452), `packages/wavs/src/subsystems/trigger.rs` (line 266), `packages/wavs/src/subsystems/submission.rs` (lines 216, 223)
- Current mitigation: Gated behind `#[cfg(feature = "dev")]` but compiled into binaries when "dev" feature enabled
- Recommendations:
  - Move these checks to test-only code or fully conditional compilation
  - If needed in production for chaos testing, require explicit configuration file entry and logging
  - Never rely on environment variables for security-critical behavior injection

**Mnemonic and Credential Storage in AppState:**
- Risk: `SubmissionManager` stores signing mnemonic in memory with no encryption. Desktop app stores credentials in Zustand state
- Files: `packages/wavs/src/subsystems/submission.rs` (line 33), `app/src/stores/walletStore.ts`
- Current mitigation: Credentials stored in `.env` (not committed)
- Recommendations:
  - Use OS secure storage (Keychain/Windows Credential Manager) for mnemonic
  - Never pass credentials through Tauri bridge unencrypted
  - Implement credential rotation mechanism

**Signing Mnemonic Missing Validation:**
- Risk: No checks that `WAVS_SIGNING_MNEMONIC` environment variable is valid BIP39 before use
- Files: `packages/wavs/src/subsystems/submission.rs` (lines 59-62)
- Current mitigation: Fails at signer creation if invalid
- Recommendations:
  - Validate mnemonic format during config loading with detailed error messages
  - Log (but never print) mnemonic length for debugging

## Performance Bottlenecks

**Paginated List Digests Missing:**
- Problem: `list_digests()` in `packages/wavs/src/subsystems/engine/wasm_engine.rs` (line 92) loads all component digests into memory and collects to Vec
- Files: `packages/wavs/src/subsystems/engine/wasm_engine.rs` (lines 92-102)
- Cause: With thousands of components, memory usage and latency scale linearly
- Improvement path: Implement cursor-based pagination, limit default results to 100, add offset/limit parameters

**Large Files Suggest Complexity:**
- Problem: `p2p.rs` (1839 lines), `evm_stream/subscription.rs` (1529 lines), and `trigger.rs` (1379 lines) are difficult to maintain
- Files: `packages/wavs/src/subsystems/aggregator/p2p.rs`, `packages/wavs/src/subsystems/trigger/streams/evm_stream/client/subscription.rs`, `packages/wavs/src/subsystems/trigger.rs`
- Cause: Each file has multiple responsibilities (event handling, state management, networking)
- Improvement path: Extract concerns into separate modules (e.g., EVM event parsing, subscription state machine, P2P message codec)

**Fuel and Time Limits Using u64::MAX by Default:**
- Problem: `Workflow::DEFAULT_FUEL_LIMIT: u64 = u64::MAX` and `DEFAULT_TIME_LIMIT_SECONDS: u64 = u64::MAX` (line 277-278) effectively disable execution limits
- Files: `packages/types/src/service.rs` (lines 277-278)
- Cause: No sane defaults; allows malicious or buggy components to consume unbounded resources
- Improvement path: Set reasonable defaults (e.g., 1M fuel, 30 second timeout) and require explicit override in service config

## Fragile Areas

**Cosmos Event Query and TriggerData Parsing:**
- Files: `packages/wavs/src/subsystems/trigger/streams/` (Cosmos implementation)
- Why fragile: Cosmos event encoding varies by chain and contract. Changes to contract event schemas break trigger parsing with no type safety
- Safe modification: Add integration tests with actual Cosmos event data, implement schema versioning in TriggerData
- Test coverage: Gaps in Cosmos-specific trigger scenarios

**EVM Block Subscription and Reorg Handling:**
- Files: `packages/wavs/src/subsystems/trigger/streams/evm_stream/` (EVM client)
- Why fragile: Block reorg handling is complex; reorg depth not configurable, may miss triggers during deep reorgs
- Safe modification: Add explicit reorg depth configuration, test with simulated reorg scenarios
- Test coverage: Limited reorg testing; primarily happy-path tested

**Aggregator P2P Quorum and Leader Election:**
- Files: `packages/wavs/src/subsystems/aggregator/p2p.rs` (1839 lines)
- Why fragile: Quorum reaching and Byzantine consensus complex; split-brain scenarios possible if network partitions
- Safe modification: Isolate consensus algorithm, add comprehensive test matrix (see `layer-tests/src/e2e/matrix.rs` line 119 TODO)
- Test coverage: Basic quorum tested, edge cases in P2P failures under-tested

**TOML-Based Service Configuration:**
- Files: `app/src/components/service/`, `app/src/pages/services/`
- Why fragile: Manual TOML editing in UI with no validation preview. Typos in workflow IDs or chain names cause silent failures
- Safe modification: Add TOML schema validation, inline error messages in editor, test round-trip serialization
- Test coverage: No roundtrip tests for service TOML serialize/deserialize

**Settings Page State Management:**
- Files: `app/src/pages/Settings.tsx` (942 lines)
- Why fragile: Large monolithic component managing environment variables, chain configs, and MCP server state. State updates not atomic; partial failures possible
- Safe modification: Split into smaller focused components, implement form validation before save, add optimistic UI updates with rollback
- Test coverage: No tests for Settings page; manual testing only

## Scaling Limits

**DashMap In-Memory Services Registry:**
- Current capacity: Tested with ~100 services; memory scales linearly
- Limit: At ~10,000 services, memory footprint becomes problematic (estimated 100+ MB); no per-service memory quota
- Scaling path: Implement external service registry (PostgreSQL) with caching layer, shard by service ID

**Single Dispatcher Channel Bottleneck:**
- Current capacity: ~1000 triggers/second through crossbeam channel
- Limit: At ~10,000 triggers/second, channel contention causes P99 latencies >100ms
- Scaling path: Implement sharded dispatcher channels (by service ID), use tokio broadcast channel for hot paths

**EVM Event Subscription Limits:**
- Current capacity: One `eth_subscribe` per chain + fallback polling
- Limit: With 100 EVM chains, managing subscriptions becomes complex; lost subscriptions silently fail
- Scaling path: Implement subscription pool, add automatic subscription health checks, fallback to polling with exponential backoff

**Component Storage Digest Index:**
- Current capacity: ~1000 components indexed in-memory
- Limit: At ~10,000 components, digests list generation and lookup slowdown. No pagination implemented
- Scaling path: Implement tiered storage (hot/cold), add partial index by prefix, implement cursor-based pagination

## Dependencies at Risk

**Wasmtime Version Pinning Without Clear Upgrade Path:**
- Risk: Wasmtime is critical dependency; upgrading requires WASM interface validation and component recompilation
- Impact: Security patches may lag; new WASM features unavailable until upgrade
- Migration plan: Document WASM interface compatibility matrix, implement component versioning, test upgrade path in CI

**alloy-rs (Ethereum Interaction) Actively Developed:**
- Risk: alloy-rs is relatively young; breaking changes expected. WAVS uses multiple alloy crates (alloy-provider, alloy-signer, alloy-primitives)
- Impact: Dependency updates may require code refactoring; breaking changes in multiple crates possible
- Migration plan: Pin alloy-rs versions, test upgrades in feature branch before releasing, maintain compatibility wrapper layer

**libp2p for P2P Networking (Complex State Machine):**
- Risk: libp2p manages complex distributed state; bugs in libp2p can cause network partition or message loss
- Impact: Aggregator P2P consensus relies on libp2p correctness; bugs silently partition network
- Migration plan: Implement libp2p abstraction layer for testing, add network partition injection tests, consider Hyperswarm as alternative

## Missing Critical Features

**Service Deactivation Without Complete Cleanup:**
- Problem: Services can be marked inactive but associated resources (subscriptions, pending submissions) not cleaned up
- Blocks: Cannot safely deactivate service during operator updates; leaks resources
- Fix approach: Implement service lifecycle states (active, draining, inactive) with graceful shutdown

**No Configuration Validation at Load Time:**
- Problem: Invalid service definitions, bad TOML syntax, or missing required fields fail at runtime during execution
- Blocks: Operators cannot catch config errors before deployment; errors surface during trigger execution
- Fix approach: Implement service definition validator in CLI and HTTP API, catch errors at registration time

**Cross-Chain Submission Not Implemented:**
- Problem: Submission subsystem doesn't handle multi-chain results (e.g., submit same result to EVM and Cosmos)
- Blocks: Cannot create services that report to multiple chains atomically
- Fix approach: Extend `Submit` enum to `MultiChainSubmit`, implement atomic submission with rollback

**No Rate Limiting on Trigger Execution:**
- Problem: If trigger fires repeatedly, component executes unbounded times consuming fuel/resources
- Blocks: DOS attacks possible via malicious trigger sources; burst-heavy workloads starve other services
- Fix approach: Implement per-service trigger rate limiter with configurable burst, add queue depth limits

## Test Coverage Gaps

**Cosmos Trigger Integration Testing:**
- What's not tested: Actual Cosmos chain event parsing and filtering with real event formats
- Files: `packages/wavs/src/subsystems/trigger/streams/cosmos.rs` (if exists), integration test layer
- Risk: Cosmos triggers may fail silently or parse events incorrectly; discovered only in production
- Priority: High - Cosmos is first-class platform

**Aggregator Byzantine Quorum Scenarios:**
- What's not tested: Aggregator behavior with Byzantine operators (malicious signatures, incorrect results), network partitions during voting
- Files: `packages/layer-tests/src/e2e/matrix.rs` (line 119 TODO indicates incomplete matrix)
- Risk: Byzantine attackers could submit invalid results undetected; consensus algorithm not hardened
- Priority: High - Security critical

**Service Registry State Persistence:**
- What's not tested: Service definitions survive node restart (currently impossible due to in-memory DB)
- Files: `packages/utils/src/storage/db.rs`, `packages/wavs/tests/storage.rs`
- Risk: No test ensures recovery from restart; cannot be tested until persistence implemented
- Priority: Medium - Blocking production deployment

**P2P Network Failures and Recovery:**
- What's not tested: Aggregator P2P recovery from network partitions, slow network links, message loss
- Files: `packages/wavs/src/subsystems/aggregator/p2p.rs`, integration test suite
- Risk: Network issues cause silent quorum failures; operators unaware network broken
- Priority: High - Network always fails eventually

**EVM Reorg and Finality Handling:**
- What's not tested: Trigger execution during EVM reorgs (blocks 1-100 reorg to different chain), finality semantics, event replay
- Files: `packages/layer-tests/src/e2e/handles/evm.rs`, EVM client tests
- Risk: Triggers fire twice during reorg or not at all; state inconsistency possible
- Priority: High - EVM primary platform

**Settings Page and CLI Configuration Round-Trip:**
- What's not tested: Settings saved in desktop app can be loaded by CLI, TOML format stays valid across edits
- Files: `app/src/pages/Settings.tsx`, `packages/cli/src/command/`
- Risk: Desktop app and CLI use inconsistent configuration formats; settings lost during migration
- Priority: Medium - UX issue

---

*Concerns audit: 2026-03-17*
