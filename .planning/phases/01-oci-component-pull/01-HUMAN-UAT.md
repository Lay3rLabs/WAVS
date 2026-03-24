---
status: partial
phase: 01-oci-component-pull
source: [01-VERIFICATION.md]
started: 2026-03-24T21:13:00Z
updated: 2026-03-24T21:13:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. End-to-end OCI deploy
expected: Service deploys successfully from `oci://` URI without local `.wasm` file; logs show pull and completion
result: [pending]

### 2. Digest mismatch rejection (OCI-03)
expected: Deploy fails with "Component digest mismatch: expected X, got Y" error when digest doesn't match
result: [pending]

### 3. Cache hit on second deploy (OCI-04)
expected: Second deploy returns immediately from cache; no re-pull from registry in logs
result: [pending]

### 4. Unpinned tag warning (OCI-05)
expected: Tag-only OCI URI deploy emits WARN log "Deploying OCI component without digest pin"
result: [pending]

### 5. Private registry authentication (OCI-06)
expected: Pull succeeds with WAVS_OCI_USERNAME/WAVS_OCI_PASSWORD set for private registry
result: [pending]

## Summary

total: 5
passed: 0
issues: 0
pending: 5
skipped: 0
blocked: 0

## Gaps
