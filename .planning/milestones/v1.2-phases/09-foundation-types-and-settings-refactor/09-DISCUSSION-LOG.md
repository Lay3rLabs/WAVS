# Phase 9: Foundation Types and Settings Refactor - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-23
**Phase:** 09-foundation-types-and-settings-refactor
**Areas discussed:** Settings reorganization, Settings visual polish, BLS type propagation

---

## Settings Reorganization

### Section Order

| Option | Description | Selected |
|--------|-------------|----------|
| Keep current order | Wallet -> Home -> TOML -> Env Vars -> MCP -> Reset. Works fine, just decompose. | ✓ |
| Group by concern | Node Config + Integrations + Security group headers | |
| Frequency-based | Most-used first, one-time setup at bottom | |

**User's choice:** Keep current order
**Notes:** No reordering needed — decomposition is the priority.

### Decomposition Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| One file per section | 6 separate components, Settings.tsx becomes wrapper | |
| Folder with index | pages/settings/ folder with index.tsx + section files | |
| You decide | Claude picks based on existing codebase patterns | ✓ |

**User's choice:** You decide
**Notes:** Claude has discretion on component structure.

### State Management

| Option | Description | Selected |
|--------|-------------|----------|
| Self-contained sections | Each section manages own state, calls Tauri directly | |
| Lifted state | Parent manages all state, passes handlers as props | |
| You decide | Claude picks cleanest approach per section | ✓ |

**User's choice:** You decide
**Notes:** Claude has discretion on state management approach.

---

## Settings Visual Polish

### Visual Direction

| Option | Description | Selected |
|--------|-------------|----------|
| Polish existing style | Keep cards, standardize spacing/padding | |
| Add section descriptions | Brief subtitles under headers | |
| You decide | Claude polishes without changing design language | ✓ |

**User's choice:** You decide
**Notes:** Claude polishes visual hierarchy.

### Navigation

| Option | Description | Selected |
|--------|-------------|----------|
| Keep scrollable | Single scrollable page, no nav | |
| Add anchor nav | Scrollable with sticky sidebar or top tabs | ✓ |
| Tabbed sections | One section visible at a time | |

**User's choice:** Add anchor nav

### Anchor Nav Style

| Option | Description | Selected |
|--------|-------------|----------|
| Left sidebar | Sticky vertical nav, content scrolls on right | ✓ |
| Top tabs | Horizontal tab bar at top | |

**User's choice:** Left sidebar
**Notes:** VS Code / GitHub Settings pattern.

---

## BLS Type Propagation

### Store Visibility

| Option | Description | Selected |
|--------|-------------|----------|
| Types only | Widen types, keep default secp256k1, no UI | ✓ |
| Types + store logic | Widen types AND update build/reverse logic | |
| Minimal | Just the type alias, nothing else | |

**User's choice:** Types only
**Notes:** Store builder/reverse logic deferred to Phase 11.

### Tauri Command Readiness

| Option | Description | Selected |
|--------|-------------|----------|
| Wired up but not called | Add TS wrappers + types, no UI calls | ✓ |
| Smoke-testable | Add wrappers + hidden debug section | |

**User's choice:** Wired up but not called
**Notes:** Phase 10 and 11 will use these commands.

---

## Claude's Discretion

- Settings decomposition file structure
- State management approach per section
- Visual polish specifics (spacing, typography, descriptions)
- P2pStatus and SignerResponse TypeScript type shapes

## Deferred Ideas

None — discussion stayed within phase scope.
