# Stack Research

**Domain:** WAVS platform additions — WIT-to-schema tooling, MCP execution interface, OCI component distribution
**Researched:** 2026-03-24
**Confidence:** HIGH (critical claims verified against official sources; crate versions confirmed from crates.io, GitHub, and docs.rs)

## Context

This document covers only the **new** crates and integration points needed for the three active features. It does not repeat the existing validated stack (wasmtime 42.0.1, rmcp 0.1, wasm-pkg-client 0.12, etc. are already in Cargo.toml).

The key discovery: WAVS already pulls components via `wasm-pkg-client` (Warg/OCI through the BytecodeAlliance toolchain), but that client routes by package namespace — not raw `oci://` URIs. The new `oci://` ComponentSource variant needs a direct OCI pull path using `oci-client` + `oci-wasm`, bypassing the wkg namespace resolution layer. Wassette v0.4.0 uses this exact combination (confirmed: `oci-client = "0.16"`, `oci-wasm = "0.4"`).

For WIT-to-schema: the Wassette `component2json` crate (Apache 2.0, on GitHub) uses `wasmparser` (not `wit-parser`) to walk component type exports at the binary level using the Wasmtime type inspection API. This is the correct approach for compiled `.wasm` binaries — `wit-parser` handles `.wit` text files, not compiled binaries. WAVS components are compiled binaries, so the path is: binary → `wasmtime::component::Component::component_type()` or `wit-component::decode()` → WIT Resolve → JSON Schema.

The `component2json` upstreaming issue (#579 on the Wassette repo) is open and unresolved (opened Nov 2025, no decision). Do not wait for upstream; implement locally using the same crates.

---

## Recommended Stack — New Additions Only

### WIT-to-Schema Tooling

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `wasmparser` | `0.245.1` | Parse WebAssembly binary component type sections | Same version Wassette's `component2json` uses. Exposes the `component-type` custom sections that encode the WIT world. Lower-level than wit-component but handles the binary format directly. |
| `wit-component` | `0.245.1` | Decode compiled `.wasm` binary → `wit_parser::Resolve` | Provides `wit_component::decode(&bytes)` which returns `(Resolve, WorldId)` — the authoritative path from binary to structured WIT types. Used by `oci-wasm 0.4.0` internally for the same purpose. **This is the right entry point.** |
| `wit-parser` | `0.245.1` | Traverse the decoded WIT Resolve to enumerate types | Exposes `Resolve`, `Interface`, `Function`, `TypeDef`, `TypeDefKind`, `Record`, `Variant`, `Enum`, `Option_`, `Result_`, `Tuple`, `List` — the complete type tree needed to generate JSON Schema. |
| `schemars` | already present | Produce JSON Schema output | Already a transitive dependency through `rmcp`. The schema generation for WIT types is hand-rolled (WIT types do not map 1:1 to Rust types), so `schemars` is used for the wrapper structure, not derivation. |

Version note: `wasmparser`, `wit-component`, and `wit-parser` are co-versioned in the `wasm-tools` monorepo. Use the same version for all three to avoid ABI mismatches. 0.245.1 is the current release as of March 2026. The `oci-wasm` crate already pins `wit-component = "0.244.0"` and `wit-parser = "0.244.0"` — if `oci-wasm` is added, align all three to 0.244.x or override in workspace.

### OCI Component Distribution

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `oci-client` | `0.16` | OCI registry pull/push client implementing OCI Distribution spec | The standard Rust OCI client (ORAS project, formerly `oci-distribution`). Wassette uses this version directly. Already implicitly used via the wasm-pkg-tools chain but not exposed for direct `oci://` URI handling. |
| `oci-wasm` | `0.4.0` | WASM-specific OCI artifact types on top of `oci-client` | Bytecode Alliance crate. Provides `WasmClient`, `WasmConfig`, and the correct OCI media types (`application/vnd.wasm.config.v0+json`, `application/wasm`). Wassette uses `oci-wasm = "0.4"` for the same use case. Thin wrapper — adds ~200 lines over `oci-client`. |

These two crates are **not** in the current workspace. The existing `wasm-pkg-client` handles the `Registry { package, domain, version, digest }` ComponentSource variant through Warg namespace resolution. The new `oci://` ComponentSource variant (`ComponentSource::Oci { uri, digest }`) requires a direct OCI pull: parse the URI, call `WasmClient::pull()`, verify the digest.

Authentication: `oci-client` uses `RegistryAuth::Anonymous` for public registries (ghcr.io public repos) and `RegistryAuth::Basic` for authenticated pulls. For v1 (pull-only public components), anonymous auth suffices.

### MCP Execution Interface

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `rmcp` | `0.1` (already in workspace) | MCP server SDK for tool registration and execution | Already present. No version change needed. The `ServerHandler` trait's `list_tools()` and `call_tool()` methods are fully dynamic — tools are returned as `Vec<Tool>` at runtime. New execution tools are added to the same `match req.name.as_ref()` dispatch in `server.rs`. |

No new MCP crates are needed. The execution interface is an extension of the existing `wavs-mcp` server. The key design constraint: the `ServerHandler` trait in `rmcp` 0.1 is already dynamic — `list_tools()` returns a `Vec<Tool>` built at call time, and `call_tool()` dispatches by name string. Adding execution tools (one per deployed service+workflow) requires no new infrastructure, only populating these existing handlers with service-derived tools.

The trust tier selection (result-only / result+signature / on-chain) is implemented as a parameter on the execution tool call, not as separate tools. This keeps the tool surface manageable regardless of how many services are deployed.

---

## Supporting Libraries — Version Verification

These are already in the workspace but their roles in the new features are noted:

| Library | Current Version | Role in New Features |
|---------|-----------------|----------------------|
| `wasmtime` | `42.0.1` | `Component::component_type()` for pre-execution type introspection. No version change needed — the API has been stable since v28. |
| `wasm-pkg-client` | `0.12.0` | Existing `Registry` ComponentSource variant continues unchanged. OCI URIs go through the new direct path, not wkg. |
| `serde_json` | `1.0.145` | JSON Schema output for WIT types. Already present. |
| `rmcp` | `0.1` | `schema_for_type::<T>()` for execution tool parameter schemas. The WIT-derived schemas are constructed manually as `serde_json::Value` and passed as `Arc<serde_json::Value>` to `Tool { input_schema }`. |

---

## Cargo.toml Changes Required

Add to `[workspace.dependencies]`:

```toml
# WIT-to-schema tooling
wasmparser = "0.245"
wit-component = "0.245"
wit-parser = "0.245"

# OCI component distribution
oci-client = "0.16"
oci-wasm = "0.4"
```

Add to the relevant package's `[dependencies]` (suggest a new `wavs-wit-schema` crate or extend `wavs-engine`):

```toml
# For WIT-to-schema
wasmparser = { workspace = true }
wit-component = { workspace = true }
wit-parser = { workspace = true }
serde_json = { workspace = true }

# For OCI pull (add to wavs-engine or a new wavs-oci crate)
oci-client = { workspace = true }
oci-wasm = { workspace = true }
```

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `wit-component::decode()` → JSON Schema | `wit-parser` for `.wit` text files | WAVS operates on compiled `.wasm` binaries, not source `.wit` files. `wit-parser` parses text format. `wit-component` decodes the binary-embedded WIT. |
| `oci-client` + `oci-wasm` for `oci://` URIs | Extend `wasm-pkg-client` | `wasm-pkg-client` routes by package namespace (e.g. `wasi:http`), not raw registry URIs. The `oci://ghcr.io/user/component:tag` format bypasses namespace resolution entirely. Adding it to wkg would mean overriding their config layer. Direct OCI client is simpler. |
| Fork/inline Wassette's `component2json` approach | Wait for Bytecode Alliance upstream | Issue #579 opened Nov 2025, no activity. Building on `wasmparser` + `wit-component` directly gives full control and avoids an external dependency. |
| Extend existing `wavs-mcp` server | New MCP server for execution | New server means new process, new configuration, user friction. The existing `ServerHandler` already supports dynamic tool registration. Extending in-place is the path of least resistance. |
| `wasmtime::component::Component::component_type()` | `wasmparser` for type walking | Both work. `wit-component::decode()` + `wit-parser::Resolve` gives a higher-level structured representation (named types, interfaces, packages) rather than raw binary section parsing. Prefer the higher-level API for maintainability. `wasmparser` is a fallback if performance becomes an issue. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `wit-bindgen` for schema generation | Generates Rust binding code at compile time, not runtime schema from arbitrary binaries | `wit-component::decode()` + `wit-parser::Resolve` for runtime introspection |
| `wasm-pkg-client` for `oci://` URI handling | Not designed for raw `oci://` URIs; uses namespace-based routing | `oci-client` + `oci-wasm` directly |
| Adding separate MCP server binary for execution | Doubles configuration surface for users; fragments the wavs-mcp interface | Extend `wavs-mcp` server with new tools in the existing `ServerHandler` |
| `jsonschema` crate | Validates existing schemas, does not generate them | Hand-roll schema construction from `wit-parser::TypeDefKind` using `serde_json::json!` macros |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `wasmparser 0.245` | `wasmtime 42.0.1` | `wasmtime` 42.x bundles its own copy of `wasmparser`. The workspace `wasmparser` is a separate dep for direct binary parsing in the schema generator — no conflict. |
| `wit-component 0.245` | `wit-parser 0.245` | Must match exactly — both live in the `wasm-tools` monorepo and share internal types. Mixing minor versions breaks compilation. |
| `oci-wasm 0.4` | `oci-client 0.16` | `oci-wasm 0.4.0` declares `oci-client = "0.16"` in its Cargo.toml. Workspace must use `oci-client = "0.16"` to avoid duplicate versions. |
| `oci-wasm 0.4` | `wit-component 0.244` | `oci-wasm 0.4.0` depends on `wit-component = "0.244.0"` and `wit-parser = "0.244.0"` for reading component exports from pulled binaries. If the workspace uses `wit-component 0.245`, Cargo will compile both. Prefer aligning to 0.245 and letting Cargo unify, or pin workspace to 0.244 if oci-wasm conflicts. This needs a `cargo tree` check at implementation time. |

---

## Integration Points

### WIT-to-schema flow

```
Uploaded/deployed .wasm bytes
  → wit_component::decode(&bytes) → (Resolve, WorldId)
  → resolve.worlds[world_id].exports
  → for each export: TypeDefKind → serde_json::Value (JSON Schema object)
  → Tool { name: normalized_function_name, input_schema: Arc<json_schema> }
```

This schema generation runs at service registration time and is cached — not on every MCP tool call. The Wasmtime `Engine` already holds compiled component artifacts; the schema pass reads the raw bytes before or during compilation.

### OCI pull flow

```
ComponentSource::Oci { uri: "oci://ghcr.io/user/component:tag", digest }
  → parse URI → (registry, repository, reference)
  → WasmClient::new(ClientConfig::default())
  → client.pull(&reference, &RegistryAuth::Anonymous).await
  → verify sha256 against digest field
  → store bytes in existing component store (same path as Download/Registry variants)
```

The `ComponentDigest` type already exists in wavs-types for digest verification.

### MCP execution trust tiers

The three tiers map directly to existing WAVS capabilities:

| Tier | Implementation | Existing Infrastructure Used |
|------|---------------|-------------------------------|
| Result only | Execute via engine, return raw bytes as base64/hex | `wavs-engine` execute path |
| Result + signature | Execute + return operator signature over result hash | `alloy-signer` HD key derivation (already used) |
| On-chain submission | Execute + sign + submit via existing aggregator/submission | Full existing pipeline |

The trust tier is a parameter (`trust_tier: "result" | "signed" | "onchain"`) on the `wavs_execute_<service_id>_<workflow_id>` tool call.

---

## Sources

- [Wassette Cargo.toml workspace deps](https://github.com/microsoft/wassette/blob/main/Cargo.toml) — confirmed oci-client 0.16, oci-wasm 0.4, rmcp 0.9.1, wasmtime 36.0.5
- [Wassette component2json Cargo.toml](https://github.com/microsoft/wassette/blob/main/crates/component2json/Cargo.toml) — confirmed wasmparser 0.245 as the parsing substrate
- [component2json upstreaming issue #579](https://github.com/microsoft/wassette/issues/579) — open since Nov 2025, no resolution
- [wit-component docs.rs](https://docs.rs/wit-component/latest/wit_component/) — version 0.245.1, `decode()` function confirmed
- [wit-parser docs.rs](https://docs.rs/wit-parser/latest/wit_parser/) — version 0.245.1 (latest), struct inventory confirmed
- [oci-wasm GitHub Cargo.toml](https://github.com/bytecodealliance/rust-oci-wasm/blob/main/Cargo.toml) — version 0.4.0, oci-client 0.16, wit-component 0.244.0 confirmed
- [oci-client docs.rs](https://docs.rs/oci-client/latest/oci_client/struct.Client.html) — pull methods and RegistryAuth confirmed, v0.16.1 (March 2026)
- [wasm-pkg-client docs.rs](https://docs.rs/wasm-pkg-client/latest/wasm_pkg_client/) — version 0.15.0, read-only registry client confirmed
- [wasmtime Component API](https://docs.wasmtime.dev/api/wasmtime/component/struct.Component.html) — `component_type()` pre-instantiation introspection confirmed
- [Microsoft OCI + WASM blog](https://opensource.microsoft.com/blog/2024/09/25/distributing-webassembly-components-using-oci-registries/) — media types `application/vnd.wasm.config.v0+json` and `application/wasm` confirmed
- Existing codebase: `/Users/jacobhartnell/Dev/projects/Layer/wavs-app-2/packages/utils/src/wkg.rs` — confirmed existing wasm-pkg-client usage pattern and why it does not cover raw OCI URIs
- Existing codebase: `/Users/jacobhartnell/Dev/projects/Layer/wavs-app-2/packages/types/src/service.rs` — confirmed `ComponentSource` variants and `Registry` struct

---
*Stack research for: WAVS WIT-to-schema, MCP execution interface, OCI component distribution*
*Researched: 2026-03-24*
