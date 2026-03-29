# Requirements: WAVS Improvements

**Defined:** 2026-03-24
**Core Value:** AI agent developers can use WAVS components as MCP tools with the same ease as Wassette, but with cryptographic trust guarantees Wassette structurally cannot provide.

## v1 Requirements

### WIT-to-Schema

- [ ] **SCHEMA-01**: Developer can run `wavs wit-schema <component.wasm>` to generate JSON Schema from a compiled component
- [ ] **SCHEMA-02**: WIT primitive types map to JSON Schema (`u32/u64` → integer, `string` → string, `bool` → boolean, `option<T>` → nullable)
- [ ] **SCHEMA-03**: WIT record and enum/variant types map to JSON Schema objects and `oneOf`
- [ ] **SCHEMA-04**: WIT doc comments are embedded as JSON Schema `description` fields
- [ ] **SCHEMA-05**: Generated schemas are cached by component SHA256 digest (skip re-parsing unchanged binaries)

### MCP Execution

- [x] **EXEC-01**: Deployed service components appear as callable MCP tools via `tools/list`
- [x] **EXEC-02**: Agent can call a component via `tools/call` and receive execution result (Tier 1: result only)
- [ ] **EXEC-03**: Agent can request signed result with operator signature proving authenticity (Tier 2)
- [ ] **EXEC-04**: Agent can request on-chain submission with transaction hash (Tier 3), gated by service-level flag in service.json
- [x] **EXEC-05**: Trust tier is an explicit `inputSchema` parameter on each tool (not parallel tools)
- [x] **EXEC-06**: MCP `notifications/tools/list_changed` fires when services are deployed or removed
- [x] **EXEC-07**: Execution tools are guarded by `--exec-enabled` flag and use `wavs_exec_` naming prefix
- [x] **EXEC-08**: Per-call timeout cap (25s) enforced at MCP layer, independent of component time limit

### OCI Distribution

- [ ] **OCI-01**: `service.json` accepts `oci://` URIs as component source
- [ ] **OCI-02**: Components are pulled from OCI registries at service deploy time
- [ ] **OCI-03**: Pulled components are verified by SHA256 digest before loading
- [ ] **OCI-04**: Pulled components are cached on disk by digest (no re-pull for identical content)
- [ ] **OCI-05**: Digest pinning (`@sha256:`) is supported; deploy warns if only tag is specified
- [ ] **OCI-06**: Authenticated pull supported via environment credentials for private registries

## v2 Requirements

### Authentication & Authorization

- **AUTH-01**: MCP HTTP transport uses ERC-8128/RFC 9421 signed requests for wallet-based authentication
- **AUTH-02**: Server recovers Ethereum address from ECDSA signature and checks per-tool authorization
- **AUTH-03**: Replay protection via TTL + optional nonce for high-value operations (Tier 3)
- **AUTH-04**: ERC-8004 on-chain identity/reputation registry integration for agent authorization

### Advanced Features

- **ADV-01**: OCI component publishing tooling (`wavs oci push`)
- **ADV-02**: WIT resource type support in schema generation
- **ADV-03**: Multi-operator quorum signing for Tier 2 (aggregate signatures from multiple operators)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Demo/doc the `Only` allowlist variant | Tracked separately, different repo |
| Wassette feature parity docs/marketing | Marketing concern, not code |
| Tauri desktop app changes | This milestone is platform/MCP focused |
| MCP stdio transport signing | Stdio is local-process; trust boundary is machine-level, not network-level |
| Custom OCI media types | Must follow CNCF spec for Wassette ecosystem compatibility |
| Blocking node startup for OCI pulls | Pull at deploy time, not boot time |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SCHEMA-01 | Phase 2 | Pending |
| SCHEMA-02 | Phase 2 | Pending |
| SCHEMA-03 | Phase 2 | Pending |
| SCHEMA-04 | Phase 2 | Pending |
| SCHEMA-05 | Phase 2 | Pending |
| EXEC-01 | Phase 3 | Complete (03-02) |
| EXEC-02 | Phase 3 | Complete (03-02) |
| EXEC-03 | Phase 3 | Pending |
| EXEC-04 | Phase 3 | Pending |
| EXEC-05 | Phase 3 | Complete (03-01) |
| EXEC-06 | Phase 3 | Complete (03-02) |
| EXEC-07 | Phase 3 | Complete (03-01) |
| EXEC-08 | Phase 3 | Complete (03-01) |
| OCI-01 | Phase 1 | Pending |
| OCI-02 | Phase 1 | Pending |
| OCI-03 | Phase 1 | Pending |
| OCI-04 | Phase 1 | Pending |
| OCI-05 | Phase 1 | Pending |
| OCI-06 | Phase 1 | Pending |

**Coverage:**
- v1 requirements: 19 total
- Mapped to phases: 19
- Unmapped: 0

---
*Requirements defined: 2026-03-24*
*Last updated: 2026-03-24 after roadmap creation (traceability populated)*
