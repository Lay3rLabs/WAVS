# Testing Patterns

**Analysis Date:** 2026-03-17

## Test Framework

**Runner:**
- Rust: `cargo test` (built-in test framework)
- No external test framework (Criterion for benchmarks only)
- Config: Implicit (tests run via `cargo test`, configured in `Cargo.toml` features)

**Run Commands:**
```bash
# Run all tests
cargo test --all

# Run tests for specific package
cargo test -p wavs

# Run integration tests only (tests/ directory)
cargo test --test dispatcher_tests

# Run with logging
RUST_LOG=debug,alloy_rpc=off cargo test -p layer-tests

# Watch mode (via external tool, not built-in)
cargo watch -x test

# Coverage (via external tool)
# Not detected - no coverage tool configured
```

**No JavaScript/TypeScript testing detected** - The codebase is primarily Rust with a Tauri/React frontend (not tested via test framework).

## Test File Organization

**Location:**
- Unit tests: Co-located in source files under `#[cfg(test)] mod tests { ... }`
- Integration tests: Separate files in `packages/[name]/tests/` directory
- E2E tests: Separate test suite in `packages/layer-tests/` with TOML configuration

**Pattern:**
- Co-located: Same file as implementation
- Separated: `tests/` directory at package root
- E2E: Dedicated package with test harness

**Examples:**
- Unit test (co-located): `packages/types/src/bytes.rs` contains `#[cfg(test)] mod tests`
- Integration test: `packages/wavs/tests/dispatcher_tests.rs`
- E2E test: `packages/layer-tests/src/` (run via `cargo test -p layer-tests`)

**Naming:**
- Test modules: `mod tests` (singular, lowercase)
- Test functions: `#[test] fn test_*` or `#[test] fn *_works` pattern
- Examples: `test_display()`, `dispatcher_pipeline()`, `send_to_self()`
- Test files: `*_tests.rs` suffix (e.g., `dispatcher_tests.rs`, `aggregator_tests.rs`)

## Test Structure

**Suite Organization:**

```rust
#[cfg(test)]
mod tests {
    use super::*;                    // Import items from parent module
    use other_test_utilities::*;     // Import test helpers

    #[test]
    fn test_basic_operation() {
        // Arrange
        let input = setup_data();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

**Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/tests/dispatcher_tests.rs`:**

```rust
#[test]
fn dispatcher_pipeline() {
    init_tracing_tests();                                    // Setup

    let data_dir = tempfile::tempdir().unwrap();            // Arrange
    let workflow_id = WorkflowId::new("workflow1").unwrap();
    let ctx = AppContext::new();
    let dispatcher = Arc::new(MockE2ETestRunner::create_dispatcher(ctx.clone(), &data_dir));

    // Register components
    let digest = dispatcher
        .engine_manager
        .engine
        .store_component_bytes(COMPONENT_SQUARE_BYTES)
        .unwrap();

    // Create service
    let service = Service { ... };

    // Spawn dispatcher thread
    std::thread::spawn({
        let dispatcher = dispatcher.clone();
        let ctx = ctx.clone();
        move || {
            dispatcher.start(ctx).unwrap();
        }
    });

    // Act: Send actions through the pipeline
    ctx.rt.block_on(async {
        dispatcher.add_service_direct(service, None).await.unwrap();
    });

    dispatcher.trigger_manager.send_actions(...);

    // Assert: Wait and verify
    wait_for_submission_messages(&submission_manager, 2, None).unwrap();
}
```

**Patterns:**
- **Setup:** `init_tracing_tests()` - initializes tracing for test logging
- **Teardown:** Implicit via `Drop` implementations or `tempfile::tempdir()` cleanup
- **Assertion:** `assert_eq!()`, `assert!()` macros
- **Async blocks:** Wrapped with `ctx.rt.block_on(async { ... })` (tokio runtime)

## Mocking

**Framework:** Custom mock implementations (not Mockito or similar)

**Patterns:**

From `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/tests/aggregator_tests.rs`:

```rust
let services = mock_services();                      // Create mock services
let service = mock_service();                        // Create mock service
let metrics = Metrics::new(...);                     // Real metrics for testing
let config = mock_config();                          // Mock configuration

let submission_manager = mock_submission_manager(    // Construct mock manager
    ctx.clone(),
    &metrics,
    &config,
    &channels,
    services.clone()
);

let aggregator = mock_aggregator(                    // Construct mock aggregator
    ctx.clone(),
    &metrics,
    &config,
    &channels,
    services
);
```

**Mock Helpers Location:** `packages/wavs/tests/wavs_systems/`

Files:
- `mock_config.rs` - Provides test configuration (signing mnemonic, defaults)
- `mock_service.rs` - Creates test services with valid structure
- `mock_aggregator.rs` - Creates aggregator with test channels
- `mock_submissions.rs` - Creates submission managers and helpers
- `mock_trigger_manager.rs` - Creates trigger managers for testing
- `channels.rs` - Test channel setup (crossbeam)
- `mock_app.rs` - Full application mock (`MockE2ETestRunner`)

**Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/tests/wavs_systems/mock_config.rs`:**

```rust
pub fn mock_config() -> Config {
    Config {
        signing_mnemonic: Some(Credential::new(
            "test test test test test test test test test test test junk".to_string(),
        )),
        ..wavs::config::Config::default()
    }
}
```

**What to Mock:**
- Configuration: Use default with test overrides for specific fields
- Services: Create minimal valid structures via factory functions
- Channels: Real crossbeam channels used for message passing tests
- External systems: Mock EVM providers, Cosmos clients (when needed)

**What NOT to Mock:**
- Core business logic (dispatcher, engine) - test with real implementations
- Type conversions and serialization - test with actual implementations
- Cryptographic operations - use test vectors, not mocks
- Time-dependent operations: Use real sleep/wait in tests (can add test mode features)

## Fixtures and Factories

**Test Data:**

From `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/tests/wavs_systems/mock_service.rs`:

```rust
pub fn mock_service() -> Service {
    Service {
        name: "Test Service".to_string(),
        workflows: [(workflow_id(), workflow())].into(),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm { ... },
    }
}

pub fn mock_services() -> Services {
    Services::new(tempfile::tempdir().unwrap().path().to_path_buf())
}
```

**Location:** `packages/wavs/tests/wavs_systems/` directory

**Pattern:**
- Factory functions return fully valid test instances
- Use sensible defaults (e.g., test mnemonic in `mock_config`)
- Support parameterization via function arguments when needed
- Reuse factories across multiple tests

**Example helper from test file:**

```rust
pub fn mock_real_trigger_action(
    service_id: ServiceId,
    workflow_id: &str,
    contract_address: &ContractAddress,
    request: &SquareRequest,
    chain: &str,
) -> TriggerAction {
    TriggerAction {
        config: TriggerActionConfig {
            service_id,
            workflow_id: WorkflowId::new(workflow_id).unwrap(),
            chain: chain.parse().unwrap(),
        },
        data: TriggerData { ... },
    }
}
```

## Coverage

**Requirements:** Not enforced
- No coverage tools detected in configuration
- No coverage requirements in CI/CD
- Coverage tracking appears to be manual/optional

**View Coverage:**
```bash
# Via tarpaulin (would need to be installed)
cargo tarpaulin --out Html

# Via llvm-cov (would need to be installed)
cargo llvm-cov --html
```

## Test Types

**Unit Tests:**
- Scope: Test single functions/methods in isolation
- Location: Co-located in `#[cfg(test)] mod tests`
- Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/types/src/bytes.rs`:
  ```rust
  #[test]
  fn test_display() {
      let data = ByteArray::<4>([0xDE, 0xAD, 0xBE, 0xEF]);
      assert_eq!(format!("{data}"), "0xdeadbeef");
  }
  ```
- Count: ~17 unit tests in `packages/types/src/`

**Integration Tests:**
- Scope: Test subsystems interacting (engine + dispatcher, submission + aggregator)
- Location: `packages/wavs/tests/` directory
- Approach: Real components with mock channel infrastructure
- Features: May use `#![cfg(feature = "dev")]` to gate test-only code
- Example: `dispatcher_tests.rs` - tests full pipeline from trigger to submission
- Files: `dispatcher_tests.rs`, `aggregator_tests.rs`, `trigger_tests.rs`, etc.

**E2E Tests:**
- Scope: Full on-chain integration with live test validators
- Location: `packages/layer-tests/` (dedicated package)
- Configuration: `layer-tests.toml` - TOML-based test selection
- Approach:
  ```toml
  [layer-tests]
  wavs_concurrency = true
  middleware_concurrency = true
  evm_middleware_type = "poa"
  mode = "all"  # or isolated test selection
  ```
- Running: `just test-wavs-e2e` or `cargo test -p layer-tests`
- Harness: `MockE2ETestRunner` in test utilities

## Common Patterns

**Async Testing:**

Pattern: Use `ctx.rt.block_on()` to run async code in tests

```rust
#[test]
fn test_async_operation() {
    let ctx = AppContext::new();

    ctx.rt.block_on(async {
        let result = async_function().await;
        assert_eq!(result, expected);
    });
}
```

From `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/tests/dispatcher_tests.rs`:
```rust
ctx.rt.block_on(async {
    dispatcher.add_service_direct(service, None).await.unwrap();
});
```

**Error Testing:**

Pattern: Use `unwrap()` to assert success, or match on `Result`

```rust
#[test]
fn test_error_case() {
    let result = function_that_may_fail();
    assert!(result.is_err());
}

#[test]
fn test_unwrap_on_success() {
    let result = function_that_should_succeed();
    let value = result.unwrap();
    assert_eq!(value, expected);
}
```

**Testing Streams and Channels:**

Pattern: Use helper functions to wait for expected messages

From `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/tests/wavs_systems/mock_submissions.rs`:

```rust
pub fn wait_for_submission_messages(
    submission_manager: &SubmissionManager,
    expected_count: usize,
    timeout: Option<Duration>,
) -> Result<()> {
    // Poll or block until expected_count messages received
}
```

Usage:
```rust
wait_for_submission_messages(&submission_manager, 2, None).unwrap();
assert_eq!(aggregator.metrics.get_broadcast_count(), 3);
```

**Testing with Tracing:**

Pattern: Initialize tracing for test output

```rust
#[test]
fn test_with_logging() {
    init_tracing_tests();  // Enables RUST_LOG env var
    // Test code here - logs will be captured
}
```

From `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/tests/aggregator_tests.rs`:
```rust
#[test]
fn send_to_self() {
    init_tracing_tests();
    // ... test continues
}
```

**Test Isolation:**

- Each test gets its own temporary directory: `tempfile::tempdir().unwrap()`
- Each test gets its own `AppContext` instance (thread-safe runtime)
- Channels are created fresh per test via `TestChannels::new()`
- No shared global state between tests

**Feature-Gated Tests:**

Tests that require dev features use `#![cfg(feature = "dev")]` at file start:

```rust
#![cfg(feature = "dev")]

#[test]
fn dispatcher_pipeline() {
    // ... test code
}
```

Run with: `cargo test --features dev`

## Test Utilities

**Location:** `packages/utils/src/test_utils/` (inferred from imports)

Common utilities:
- `init_tracing_tests()` - Setup tracing infrastructure
- `AppContext::new()` - Create test app context with tokio runtime
- `mock_contracts::*` - Test contract ABIs and types
- `test_utils::address::rand_address_evm()`, `rand_address_cosmos()` - Random test addresses
- `mock_engine::COMPONENT_SQUARE_BYTES` - Pre-compiled test WASM components

**Usage Pattern:**

```rust
use utils::init_tracing_tests;
use utils::context::AppContext;
use utils::test_utils::{
    address::{rand_address_cosmos, rand_address_evm},
    mock_engine::COMPONENT_SQUARE_BYTES,
};

#[test]
fn test_something() {
    init_tracing_tests();
    let ctx = AppContext::new();
    let addr = rand_address_evm();
    // ... test continues
}
```

---

*Testing analysis: 2026-03-17*
