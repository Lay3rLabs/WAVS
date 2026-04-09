# Phase 16: Wallet Kebab Menu - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Uncommon wallet actions (Export Recovery Phrase, Reset Wallet) move from inline buttons to a kebab (three-dot) dropdown menu in the wallet section header. Existing behaviors remain identical.

</domain>

<decisions>
## Implementation Decisions

### Kebab Menu Placement
- Three-dot icon (⋮) in the top-right of the wallet card header, next to the "Wallet" heading
- Click opens a small dropdown aligned to the right
- Dropdown contains: "Export Recovery Phrase" and "Reset Wallet" items
- "Reset Wallet" shown in red text to indicate destructive action
- Dropdown closes on outside click or after selecting an action

### Behavior Preservation
- Export Recovery Phrase triggers the same handleExportWallet flow (getMnemonic → show grid)
- Reset Wallet triggers the same handleResetWallet flow (confirm dialog → deleteMnemonic)
- The mnemonic display area and reset confirmation dialog remain inline in the card (not in the dropdown)
- Only the trigger buttons move into the kebab — all confirmation/display UI stays in place

### Claude's Discretion
- Whether to use a simple div-based dropdown or a more sophisticated approach
- Click-outside dismiss implementation (useRef + useEffect or simpler approach)
- Exact icon implementation (Unicode ⋮, SVG, or CSS dots)
- Dropdown animation (instant show/hide vs fade)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `WalletSection.tsx` — main component to modify (lines 228-277 contain the buttons to move)
- `Button` component from `../atoms` — currently used for the actions
- Existing state: `showMnemonic`, `showResetConfirm` — control inline display areas

### Established Patterns
- Tailwind utility classes for styling
- useState for local UI state
- bg-charcoal-dark/darkest for dropdown background
- border-charcoal-light for borders

### Integration Points
- WalletSection.tsx — only file that needs modification
- No new components needed — kebab can be inline in WalletSection

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the approved layout.

</specifics>

<deferred>
## Deferred Ideas

None — simple UI reorganization.

</deferred>
