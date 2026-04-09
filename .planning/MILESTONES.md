# Milestones

## v1.3 Activity UX & Bug Fixes (Shipped: 2026-04-09)

**Phases completed:** 4 phases, 4 plans, 2 tasks

**Key accomplishments:**

- One-liner:
- 1. [Rule 1 - Bug] Fixed pre-existing missing exec_enabled field in block_interval test

---

## v1.2 Components Explorer (Shipped: 2026-04-08)

**Phases completed:** 3 phases, 4 plans, 4 tasks

**Key accomplishments:**

- TypeScript types, Tauri command wrappers, useComponentDetail hook, and ComponentDetailPage shell with breadcrumb, header card, and tab navigation at /components/:digest
- One-liner:

---

## v1.1 Open Source AI Providers & Settings UX (Shipped: 2026-04-08)

**Phases completed:** 3 phases, 3 plans, 6 tasks

**Key accomplishments:**

- Groq and OpenRouter added as selectable agent providers with dynamic model placeholders and settings-aware sidecar startup via settings.json read at startup.
- Ollama added as selectable agent provider with conditional base URL field, models.json generation from Rust backend, and ModelRegistry.create() sidecar switch for OpenAI-compatible local model support

---

## v1.0 WAVS Improvements (Shipped: 2026-04-07)

**Phases completed:** 6 phases, 12 plans, 23 tasks

**Key accomplishments:**

- ComponentSource::Oci variant with optional digest and OCI puller module using oci-client 0.16 / oci-wasm 0.4 for authenticated WASM component pulls
- OCI pull wired into engine pipeline with tuple return, digest verification, cache-hit optimization, unpinned-tag warning, and all 10 call sites updated for Option<&ComponentDigest>
- WIT-to-JSON Schema conversion library with recursive type mapping, $defs deduplication, digest-based caching, and WIT doc comment enrichment
- Execution types, error codes, schema merging, service cache, ExecContext, --exec-enabled flag, and POST /dev/execute endpoint for synchronous component result retrieval
- End-to-end MCP execution pipeline: dynamic tool discovery from deployed services via list_tools(), Tier 1 result_only dispatch via call_tool() with timeout enforcement, and peer-based list_changed notifications on service CRUD
- Three trust tiers complete: signed_result returns operator EIP-191 signature with HD-derived key; on_chain implements two-step estimate/submit flow via EvmSigningClient with real tx_hash
- 1. [Rule 2 - Missing null checks] Updated ActivityCard.tsx and ActivityFeed.tsx for optional triggerData
- 1221-line monolithic Settings.tsx decomposed into 4 isolated section components + SettingsSidebar + 615-line orchestrating shell with sidebar navigation and parent OAuth listener
- One-liner:
- useGroupedActivity hook with single-pass correlationId grouping and appStore ERR-02 eviction guard preserving failed events from FIFO removal
- GroupedActivityCard component and ActivityFeed refactor delivering nested trigger-submission cards with amber/red status dots, full error display, and status-based filter tabs replacing kind-based tabs

---
