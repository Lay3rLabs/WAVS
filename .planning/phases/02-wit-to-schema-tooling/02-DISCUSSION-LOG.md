# Phase 2: WIT-to-Schema Tooling - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-25
**Phase:** 02-wit-to-schema-tooling
**Areas discussed:** Variant/enum mapping, Schema scope & structure, Doc comment extraction, CLI output & UX

---

## Variant/Enum Mapping

### WIT variant representation

| Option | Description | Selected |
|--------|-------------|----------|
| Externally tagged | Each variant case is a oneOf entry with the case name as a required property key. Matches serde defaults and Wassette. | ✓ |
| Discriminator field | Explicit "type" discriminator field + payload field. More explicit but diverges from serde. | |
| You decide | Claude picks based on Wassette compatibility and serde alignment | |

**User's choice:** Externally tagged (Recommended)
**Notes:** None

### u128 representation

| Option | Description | Selected |
|--------|-------------|----------|
| String type | Map u128 to string with regex pattern. Standard in blockchain tooling. | ✓ |
| String without pattern | Just string type with description. Simpler but less validation. | |
| You decide | Claude picks | |

**User's choice:** String type (Recommended)
**Notes:** None

### WIT enum handling

| Option | Description | Selected |
|--------|-------------|----------|
| String enum | Map WIT enum to string enum array. Clean, standard, distinct from variant/oneOf. | ✓ |
| Unified with variants | Treat enums as variants with no payloads. More consistent internally but verbose. | |

**User's choice:** String enum (Recommended)
**Notes:** None

---

## Schema Scope & Structure

### Export scope

| Option | Description | Selected |
|--------|-------------|----------|
| All exports in one schema | Single JSON object with all exported functions, inputSchema and outputSchema per function. | ✓ |
| Per-function flag | Default all, --function flag for single function. | |
| You decide | Claude picks based on Phase 3 MCP needs | |

**User's choice:** All exports in one schema (Recommended)
**Notes:** None

### Import inclusion

| Option | Description | Selected |
|--------|-------------|----------|
| Exports only | Only exported functions. Imports are runtime details. | ✓ |
| Both with separation | Include imports and exports in separate sections. | |
| You decide | Claude picks | |

**User's choice:** Exports only (Recommended)
**Notes:** None

### Type deduplication

| Option | Description | Selected |
|--------|-------------|----------|
| $defs with $ref | Shared types defined once in $defs, referenced via $ref. Standard JSON Schema practice. | ✓ |
| Inline everything | Fully expand all types. Simpler but can be very large. | |

**User's choice:** $defs with $ref (Recommended)
**Notes:** None

---

## Doc Comment Extraction

### Extraction strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Binary-first, fallback gracefully | Try Wasmtime API first. If docs unavailable, emit schema without descriptions. --wit-path flag for WIT source enrichment. | ✓ |
| Require WIT source | Always require --wit-path. Guarantees SCHEMA-04 but adds friction. | |
| You decide | Claude picks based on what Wasmtime API exposes | |

**User's choice:** Binary-first, fallback gracefully (Recommended)
**Notes:** None

---

## CLI Output & UX

### Output behavior

| Option | Description | Selected |
|--------|-------------|----------|
| JSON to stdout always | Always JSON Schema to stdout. Diagnostics to stderr. Pipe-friendly. | ✓ |
| Pretty JSON + --compact | Default pretty-printed, --compact flag for minified. | |
| You decide | Claude picks based on existing CLI patterns | |

**User's choice:** JSON to stdout always (Recommended)
**Notes:** None

### Library crate organization

| Option | Description | Selected |
|--------|-------------|----------|
| Separate library crate | New packages/wit-schema/ crate. CLI and Phase 3 MCP both import it. | |
| CLI-only, extract later | Build in CLI first, extract when Phase 3 needs it. | |
| You decide | Claude picks based on codebase patterns and Phase 3 dependency | ✓ |

**User's choice:** You decide (Claude's Discretion)
**Notes:** None

---

## Claude's Discretion

- Library crate organization (separate crate vs CLI-only with later extraction)
- Cache implementation details (LRU vs disk)
- Error formatting and exit codes
- WIT result<T, E> and option<T> mapping details

## Deferred Ideas

None
