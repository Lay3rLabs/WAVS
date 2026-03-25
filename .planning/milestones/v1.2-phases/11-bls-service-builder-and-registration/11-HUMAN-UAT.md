---
status: partial
phase: 11-bls-service-builder-and-registration
source: [11-VERIFICATION.md]
started: 2026-03-24T00:00:00Z
updated: 2026-03-24T00:00:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. BLS Algorithm Selection Propagates to Deployed Service
expected: Open service builder, set Submit Type to Aggregator, change Signature Algorithm to BLS. Prefix dropdown auto-resets to "None" immediately.
result: [pending]

### 2. Post-Deploy BLS Key Card Appears
expected: Deploy a BLS service to running WAVS node. After deploy completes, "BLS Operator Key" card appears with G1 pubkey and copy-to-clipboard.
result: [pending]

### 3. One-Click BLS Registration Flow
expected: On service detail page for BLS service, click "Register BLS Key". Button shows "Registering...", then badge flips to "Registered".
result: [pending]

### 4. Registration Status Read from On-Chain State
expected: Service detail page for BLS service shows correct on-chain registration status badge without manual refresh.
result: [pending]

## Summary

total: 4
passed: 0
issues: 0
pending: 4
skipped: 0
blocked: 0

## Gaps
