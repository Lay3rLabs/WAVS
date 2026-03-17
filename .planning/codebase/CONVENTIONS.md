# Coding Conventions

**Analysis Date:** 2026-03-17

## Naming Patterns

**Files:**
- Rust: `snake_case` (e.g., `dispatcher.rs`, `service_registry.rs`, `mock_config.rs`)
- TypeScript/TSX: `camelCase` or `PascalCase` for components (e.g., `App.tsx`, `appStore.ts`, `AddressDisplay.tsx`)
- Test modules: inline in Rust files under `#[cfg(test)] mod tests { ... }`
- Test files: separate files in `tests/` directory with `_tests.rs` suffix (e.g., `dispatcher_tests.rs`, `aggregator_tests.rs`)

**Functions:**
- Rust: `snake_case` for all functions (pub and private)
  - Examples: `add_service()`, `remove_service_inner()`, `emit_ext()`, `store_component_bytes()`
  - Async: `async fn add_service()` - same naming, prefix with `async` keyword
- TypeScript: `camelCase` for functions and methods
  - Examples: `errorMessage()`, `buildServiceMap()`, `getServiceLabel()`
  - React hooks: `use` prefix (e.g., `useAppStore()`, `useWalletStore()`, `useEffect()`)

**Variables:**
- Rust: `snake_case` for all variables (let bindings, struct fields, locals)
  - Examples: `service_id`, `workflow_id`, `dispatcher_handle`, `services`
- TypeScript: `camelCase` for variables and state
  - Examples: `isSettingsComplete`, `hasMnemonic`, `wavsStarted`, `settings`

**Types:**
- Rust: `PascalCase` for types, traits, structs, enums
  - Examples: `Service`, `Dispatcher`, `TriggerManager`, `WavsSignable`, `AggregatorInput`
  - Error types: `*Error` suffix (e.g., `ServiceError`, `DispatcherError`, `EngineError`)
- TypeScript: `PascalCase` for interfaces, types, components
  - Examples: `Settings`, `LogItem`, `ActivityItem`, `Service`, `TextInputProps`

**Constants:**
- Rust: `SCREAMING_SNAKE_CASE` for compile-time constants and module-level statics
- TypeScript: `SCREAMING_SNAKE_CASE` or `camelCase` depending on scope
  - Examples: `MAX_LOG_ITEMS`, `MAX_ACTIVITY_ITEMS` (module-level); `host` (env-derived)

## Code Style

**Formatting:**
- Rust: Enforced by `cargo fmt` (configured via workspace)
  - Run: `just lint` (check), `just lint-fix` (auto-fix)
  - Line length: Standard (default is 99 characters)
  - Indentation: 4 spaces

- TypeScript: No explicit linter configured, but follows standard conventions
  - Indentation: 2 spaces (inferred from existing code)
  - Target: ES2020, strict TypeScript mode enabled

**Linting:**
- Rust: `cargo clippy --all-targets --all-features` with `-D warnings` (deny all warnings)
  - Suppressed lints in library root: `clippy::result_large_err`, `clippy::uninlined_format_args`, `clippy::type_complexity`
  - Run: `just lint` (check), `just lint-fix` (auto-fix)

- TypeScript: Strict compiler settings (tsconfig.json)
  - `strict: true` - all strict flags enabled
  - `noUnusedLocals: true` - error on unused variables
  - `noUnusedParameters: true` - error on unused function parameters
  - `noFallthroughCasesInSwitch: true` - error on incomplete switch statements

## Import Organization

**Rust Order:**
1. `use std::*` - Standard library imports
2. `use crate::*` - Internal crate imports and relative paths
3. `use external_crate::*` - Workspace and external dependencies (alphabetically grouped)
4. Conditional compilation blocks (`#[cfg(...)]`) for feature-gated imports

**Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/types/src/signing.rs`:**
```rust
use crate::{...};
use alloy_primitives::{...};
use alloy_sol_types::SolValue;
use async_trait::async_trait;
use ripemd::Ripemd160;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use thiserror::Error;
use utoipa::ToSchema;
```

**TypeScript Order:**
1. Third-party/library imports (React, DOM, external packages)
2. Local imports from `./` (relative paths)
3. Type imports (`import type { ... }`)

**Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/app/src/App.tsx`:**
```typescript
import { useEffect, useState } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Header, Body } from './components/layout';
import { useAppStore } from './stores/appStore';
import { buildServiceMap } from './types';
```

**Path Aliases:**
- TypeScript: Not configured - uses relative paths throughout
- Rust: Uses workspace dependencies via `Cargo.toml` references (e.g., `use wavs::...`, `use utils::...`)

## Error Handling

**Rust Patterns:**

1. **Result types:** Use `Result<T>` from `anyhow` or `thiserror` throughout
   - Functions propagate errors with `?` operator
   - Example: `pub fn hash(&self) -> Result<ServiceDigest, ServiceError>`

2. **Error definitions:** Custom enum types with `#[derive(thiserror::Error)]`
   - Located in `error.rs` modules or inline
   - Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/src/http/error.rs`:
   ```rust
   #[derive(thiserror::Error, Debug)]
   pub enum HttpError {
       #[error("not found")]
       NotFound,
   }
   ```

3. **Error conversion:** `From<E> for AppError` implementations enable `?` chaining across error types
   ```rust
   impl<E> From<E> for AnyError
   where
       E: Into<anyhow::Error>,
   {
       fn from(err: E) -> Self {
           Self(err.into())
       }
   }
   ```

4. **HTTP error handling:** Custom wrapper types implementing `IntoResponse` for Axum
   - Downcasts errors to check for specific types
   - Returns appropriate HTTP status codes

**TypeScript Patterns:**

1. **Error extraction:** Helper function in `/Users/jacobhartnell/Dev/WAVS/WAVS/app/src/utils/error.ts`
   ```typescript
   export function errorMessage(e: unknown): string {
     if (typeof e === 'string') return e;
     if (e instanceof Error) return e.message;
     if (typeof e === 'object' && e !== null) {
       const values = Object.values(e as Record<string, unknown>);
       if (values.length > 0 && typeof values[0] === 'string') return values[0];
       return JSON.stringify(e);
     }
     return String(e);
   }
   ```

2. **Tauri command errors:** Commands reject with serialized error enums
   - Format: `{"ErrorVariant": "description"}` or `{"ErrorVariant": null}`
   - Extracted using the above helper

3. **Async/await:** Try-catch blocks in async functions with error logging
   - Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/app/src/App.tsx`:
   ```typescript
   try {
     const settings = await getSettings();
     setSettings(settings);
   } catch (err) {
     console.error('Failed to initialize app:', err);
     setError(String(err));
   }
   ```

## Logging

**Rust Framework:** `tracing` crate

- Initialization: `init_tracing_tests()` in tests
- Levels: `trace!()`, `debug!()`, `info!()`, `warn!()`, `error!()`
- Structured logging: Named fields via macro arguments
  ```rust
  tracing::info!("message with field: {field_name}");
  tracing::debug!("Jaeger tracing enabled");
  ```
- Instrumentation: `#[instrument]` macro on functions to auto-trace entry/exit
- Configuration: Via `RUST_LOG` environment variable (e.g., `RUST_LOG=debug,wavs=debug`)

**TypeScript Framework:** `console` object

- Methods: `console.log()`, `console.warn()`, `console.error()`
- Used for initialization and error reporting
- Example: `console.warn('Failed to start WAVS:', err);`
- No structured logging framework detected

## Comments

**When to Comment:**
- Multi-line documentation comments for public APIs and complex functions
- Explain the "why", not the "what" (code shows the "what")
- Document runtime invariants and constraints

**Documentation Style (Rust):**
- Three-slash doc comments (`///`) for public items
- Doc comment blocks before items they document
- Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/types/src/bytes.rs`:
  ```rust
  /// A newtype that wraps a `[u8; N]` using const generics.
  /// and is serialized as a `0x` prefixed hex string.
  #[derive(Clone, PartialEq, Eq, Hash, Copy, ToSchema, bincode::Encode, bincode::Decode)]
  pub struct ByteArray<const N: usize>([u8; N]);
  ```

- Field documentation (inline docs):
  ```rust
  /// This is any utf-8 string, for human-readable display.
  pub name: String,

  /// We support multiple workflows in one service with unique service-scoped IDs.
  pub workflows: BTreeMap<WorkflowId, Workflow>,
  ```

- Enum variant docs:
  ```rust
  /// Create a new record
  Create,
  /// Update an existing record
  Update,
  /// Delete a record
  Delete,
  ```

**Documentation Style (TypeScript):**
- JSDoc-style blocks (/** ... */) for exported functions
- Single-line comments for inline explanation
- Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/app/src/utils/error.ts`:
  ```typescript
  /**
   * Extract a human-readable message from any thrown value.
   *
   * Tauri commands reject with the serialized AppError enum, e.g.:
   *   {"Service": "wavs-mcp binary not found"}
   *   {"WavsNotRunning": null}
   * Plain strings and Error objects are handled too.
   */
  export function errorMessage(e: unknown): string { ... }
  ```

**Line Comments:**
- Explain complex logic or non-obvious decisions
- Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/src/dispatcher.rs`:
  ```rust
  // Wait until the HTTP server has bound its port before running the restore loop,
  // so that fetches of local dev-service URIs don't get "connection refused".
  ```

## Function Design

**Size:**
- Rust: Functions are generally 10-50 lines, larger functions broken into smaller helpers
- TypeScript: React components 30-100 lines, hooks/utils 5-30 lines

**Parameters:**
- Rust: Use struct types for functions with many parameters
  - Avoid excessive boolean/enum flags
  - Example: `Service` struct contains `name`, `workflows`, `status`, `manager`
- TypeScript: Object destructuring for multiple parameters in components
  - Example: `function AddressDisplay({ address, full = false, className }: AddressDisplayProps)`

**Return Values:**
- Rust: Explicit `Result<T, E>` types for fallible operations
  - Functions that can fail return `Result`
  - Example: `pub fn hash(&self) -> Result<ServiceDigest, ServiceError>`
- TypeScript: Explicit return types on public functions, implicit on internal helpers
  - React components return `JSX.Element` or `React.ReactNode`
  - Async functions explicitly typed with `Promise<T>`

## Module Design

**Exports:**
- Rust: Use `pub mod` for public modules, re-export important items at crate root
  - Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/src/lib.rs`:
  ```rust
  pub mod args;
  pub mod config;
  pub mod dispatcher;
  pub mod subsystems;
  ```

- TypeScript: Named exports for all functions/types, use `export { }` at end of files
  - No default exports except for React components in component files
  - Example: `export function useAppStore(...)`

**Barrel Files:**
- Rust: Minimal re-exports; subsystems define their own `mod.rs` or `lib.rs`
  - No wildcard re-exports (`pub use *`)
  - Explicit about what's public API
- TypeScript: Component directories have `index.ts` that re-exports (rarely used)
  - Most imports are direct (e.g., `import { Header } from './components/layout'`)

**Visibility:**
- Rust: Explicit `pub` on items meant for external use, private by default
  - Methods on public types are `pub`
  - Helper functions are private (`fn`)
- TypeScript: All exports are public by convention; no `private` keyword on module level
  - Component props interfaces are exported for consumers
  - Internal utilities marked with comments or placed in internal folders

---

*Convention analysis: 2026-03-17*
