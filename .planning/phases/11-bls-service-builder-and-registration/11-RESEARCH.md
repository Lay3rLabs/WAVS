# Phase 11: BLS Service Builder and Registration - Research

**Researched:** 2026-03-24
**Domain:** Tauri 2 + React 19 frontend -- BLS algorithm selection in service builder, post-deploy BLS pubkey display, one-click BLS key on-chain registration, service detail BLS status
**Confidence:** HIGH

## Summary

Phase 11 adds BLS-specific features to the existing service builder and service detail pages. The backend infrastructure from Phase 9 (types, Tauri commands) and Phase 10 (operator key display, registration checks) provides the foundation. The phase has four requirements: (1) algorithm selector in the submit editor, (2) post-deploy BLS pubkey display, (3) one-click BLS key registration on-chain, (4) BLS registration status on the service detail page.

The most technically significant work is BLS-03 (one-click registration). The BLS POA middleware contract's `updateOperatorSigningKey(bytes blsKey, bytes blsSigProof)` takes TWO arguments: a 128-byte G1 pubkey and a 256-byte G2 proof-of-possession. The proof is a BLS signature of `keccak256(abi.encode(operator))`. This means the existing `cmd_derive_bls_pubkey` Tauri command is NOT sufficient for registration -- it only returns the G1 pubkey. A new Tauri command is needed: `cmd_bls_sign_proof_of_possession` that takes the operator's EVM address and HD index, derives the BLS private key, and produces the 256-byte G2 proof. The on-chain transaction itself must be sent from the operator's EVM wallet (HD index 0) -- the existing wallet infrastructure handles this.

Critical insight: the ECDSA and BLS POA middleware contracts have DIFFERENT `updateOperatorSigningKey` function signatures. The ECDSA version takes `(address newSigningKey, bytes signingKeySignature)`, while the BLS version takes `(bytes blsKey, bytes blsSigProof)`. The frontend needs a BLS-specific ABI entry for the BLS contract. The existing `POAStakeRegistryABI` in `app/src/contracts/POAStakeRegistry.ts` has the ECDSA version. A BLS ABI variant must be added.

For BLS-01 (algorithm selector), the `SubmitEditor` component currently shows Submit Type and Signature Prefix dropdowns. Adding a Signature Algorithm dropdown (ECDSA/BLS) is straightforward. When BLS is selected, the Signature Prefix should auto-set to 'none' (BLS does not use EIP-191 prefix).

For BLS-02 (post-deploy pubkey display), after `ServiceDeploy` completes, if the service used BLS algorithm, call `deriveBlsPubkey` to show the G1 pubkey with copy-to-clipboard. The HD index comes from the service signer response.

For BLS-04 (registration status on detail page), the approach mirrors Phase 10's P2P page but on the service detail page. For BLS services, read `getLatestOperatorSigningKey(address)` which returns `bytes` (128-byte BLS key) on the BLS contract, vs `address` on the ECDSA contract.

**Primary recommendation:** Split into two plans: (1) Algorithm selector + post-deploy key display (BLS-01, BLS-02 -- pure UI wiring with existing commands), (2) BLS key registration + status display (BLS-03, BLS-04 -- requires new Tauri command for proof-of-possession and BLS ABI).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BLS-01 | Algorithm selector (ECDSA/BLS) in service builder submit step | `SubmitEditor.tsx` already renders submit type + prefix dropdowns. Add `signatureAlgorithm` dropdown using existing `Dropdown` component. `SubmitDraft.signatureAlgorithm` is already `SignatureAlgorithm` (widened in Phase 9). When BLS selected, auto-set prefix to 'none'. |
| BLS-02 | Post-deploy BLS G1 pubkey display with copy-to-clipboard | `ServiceDeploy.tsx` has `onDeployComplete` callback. After deploy, if algorithm is BLS, call `deriveBlsPubkey(hdIndex)` from `tauri/commands.ts`. Display result with `AddressDisplay` component. HD index comes from `getServiceSigner()`. |
| BLS-03 | One-click BLS key registration on-chain (calls `updateOperatorSigningKey` on BLS registry) | Requires NEW Tauri command `cmd_bls_sign_proof_of_possession(operator_address, hd_index)` that produces the 256-byte G2 proof-of-possession. Also requires BLS-specific ABI entry in `POAStakeRegistry.ts` since the BLS contract's `updateOperatorSigningKey` has different parameter types (`bytes, bytes` vs `address, bytes`). Transaction sent from operator wallet (HD index 0). |
| BLS-04 | BLS registration status shown on service detail page | Read `getLatestOperatorSigningKey(operator)` from the BLS POA registry. Returns `bytes` (not `address`). Check if non-empty to determine registered status. Reuse `RegistrationBadge` pattern from P2P page (Phase 10). |
</phase_requirements>

## Standard Stack

### Core (already installed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19.1.0 | UI framework | Already in use |
| Zustand | 5.0.0 | State management (serviceBuilderStore) | Already in use |
| viem | 2.23.5 | On-chain reads and writes | Already in use for all contract interactions |
| @tauri-apps/api | 2.10.1 | Tauri IPC | Already in use for invoke() |
| Tailwind CSS | 3.4.0 | Styling | Already in use |

### No New Dependencies

This phase requires zero new npm or cargo packages. All functionality is available via existing stack.

## Architecture Patterns

### Recommended Project Structure

```
app/src/
  components/service/
    SubmitEditor.tsx           # MODIFY: Add algorithm selector dropdown
    ServiceDeploy.tsx          # MODIFY: Add post-deploy BLS key display
  pages/services/
    ServiceDetailPage.tsx      # MODIFY: Add BLS registration status + register button
  contracts/
    POAStakeRegistry.ts        # MODIFY: Add BLS-specific ABI entries
  tauri/
    commands.ts                # MODIFY: Add blsSignProofOfPossession wrapper
  types/
    index.ts                   # MODIFY: Add BlsProofResponse type
  utils/
    evm.ts                     # MODIFY: Add updateBlsSigningKey function
app/src-tauri/src/
  commands.rs                  # MODIFY: Add cmd_bls_sign_proof_of_possession
  lib.rs                       # MODIFY: Register new command
```

### Pattern 1: Algorithm Selector in SubmitEditor

**What:** Add a SignatureAlgorithm dropdown alongside the existing Submit Type dropdown.
**When to use:** When submit type is 'aggregator'.
**Source:** Follows existing `Dropdown` pattern in `SubmitEditor.tsx`

```typescript
import type { SignatureAlgorithm } from '../../types';

type SigAlgorithm = SignatureAlgorithm;

const ALGORITHM_OPTIONS: DropdownOption<SigAlgorithm>[] = [
  { label: 'ECDSA (secp256k1)', value: 'secp256k1' },
  { label: 'BLS (bls12381)', value: 'bls12381' },
];

// Inside the aggregator section of SubmitEditor:
<div className="flex flex-col gap-2">
  <label className="text-beige-warm text-sm">Signature Algorithm</label>
  <Dropdown
    options={ALGORITHM_OPTIONS}
    value={submit.signatureAlgorithm}
    onChange={(v) => {
      const updates: Partial<SubmitDraft> = { signatureAlgorithm: v };
      // BLS does not use EIP-191 prefix
      if (v === 'bls12381') updates.signaturePrefix = 'none';
      update(updates);
    }}
    size="sm"
  />
</div>
```

### Pattern 2: Post-Deploy BLS Key Display

**What:** After service deployment completes, show the operator's BLS G1 pubkey if the service uses BLS.
**When to use:** In `ServiceDeploy.tsx` after all deploy steps succeed for a BLS service.
**Source:** `deriveBlsPubkey` from Phase 9, `AddressDisplay` for copy-to-clipboard.

```typescript
// After deploy completes successfully, if BLS:
const [blsPubkey, setBlsPubkey] = useState<string | null>(null);

// In handleDeploy, after registerStatus: 'done':
if (service && isBls) {
  try {
    const signerResp = await getServiceSigner(resolvedManager.manager);
    if ('bls12381' in signerResp) {
      setBlsPubkey(signerResp.bls12381.g1_pubkey_hex);
    }
  } catch {
    // Try deriving directly
    const resp = await deriveBlsPubkey(0);
    setBlsPubkey(resp.g1_pubkey_hex);
  }
}

// In render, after deploy steps:
{blsPubkey && (
  <div className="p-4 rounded bg-charcoal-medium border border-charcoal-light">
    <h4 className="text-beige-warm text-sm font-medium mb-2">BLS Operator Key</h4>
    <AddressDisplay address={`0x${blsPubkey}`} />
    <p className="text-tan-muted text-xs mt-2">
      Register this key on-chain from the service detail page.
    </p>
  </div>
)}
```

### Pattern 3: BLS Proof-of-Possession (New Tauri Command)

**What:** Generate a 256-byte G2 BLS signature proving ownership of the BLS key, used for on-chain registration.
**When to use:** When registering a BLS operator key on-chain.
**Source:** `bls_signing.rs` has `bls_sign_digest()` which produces exactly this.

The BLS POA contract's `updateOperatorSigningKey(bytes blsKey, bytes blsSigProof)` requires:
- `blsKey`: 128-byte G1 pubkey (from `cmd_derive_bls_pubkey`)
- `blsSigProof`: G2 signature of `keccak256(abi.encode(operator_address))` -- 256 bytes

```rust
// New Tauri command in commands.rs
#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_bls_sign_proof_of_possession(
    mnemonic_cache: State<'_, MnemonicCacheState>,
    hd_index: u32,
    operator_address: String,
) -> AppResult<BlsProofResponse> {
    let mnemonic = get_mnemonic_cached(&mnemonic_cache)
        .ok_or_else(|| AppError::Keychain("No mnemonic found".to_string()))?;
    let key = utils::bls_signing::bls_private_key_from_mnemonic(&mnemonic.to_string(), hd_index)
        .map_err(|e| AppError::Service(format!("BLS key derivation failed: {}", e)))?;

    // Compute the same digest the contract expects: keccak256(abi.encode(operator))
    let operator: alloy_primitives::Address = operator_address
        .parse()
        .map_err(|e| AppError::Service(format!("Invalid operator address: {}", e)))?;
    let encoded = alloy_sol_types::SolValue::abi_encode(&(operator,));
    let digest: [u8; 32] = alloy_primitives::keccak256(&encoded).into();

    let proof = utils::bls_signing::bls_sign_digest(&key, &digest)
        .map_err(|e| AppError::Service(format!("BLS proof signing failed: {}", e)))?;

    // Also return the G1 pubkey for convenience
    let g1_bytes = utils::bls_signing::bls_g1_pubkey_bytes(&key)
        .map_err(|e| AppError::Service(format!("G1 pubkey derivation failed: {}", e)))?;

    Ok(BlsProofResponse {
        g1_pubkey_hex: const_hex::encode(g1_bytes),
        g2_proof_hex: const_hex::encode(proof),
    })
}
```

```typescript
// TypeScript type
export interface BlsProofResponse {
  g1_pubkey_hex: string;  // 128 bytes = 256 hex chars
  g2_proof_hex: string;   // 256 bytes = 512 hex chars
}

// TypeScript wrapper
export async function blsSignProofOfPossession(
  hdIndex: number,
  operatorAddress: string
): Promise<BlsProofResponse> {
  return invoke<BlsProofResponse>('cmd_bls_sign_proof_of_possession', {
    hd_index: hdIndex,
    operator_address: operatorAddress,
  });
}
```

### Pattern 4: BLS ABI for Registration

**What:** The BLS POA middleware contract has a different `updateOperatorSigningKey` signature than the ECDSA version.
**When to use:** When calling the BLS registry contract to register a BLS signing key.
**Source:** `packages/types/src/contracts/solidity/abi/bls/IPOAStakeRegistry.json`

ECDSA contract: `updateOperatorSigningKey(address newSigningKey, bytes signingKeySignature)`
BLS contract: `updateOperatorSigningKey(bytes blsKey, bytes blsSigProof)`

```typescript
// Add to POAStakeRegistry.ts
export const BLSPOAStakeRegistryABI = [
  // updateOperatorSigningKey for BLS: (bytes blsKey, bytes blsSigProof)
  {
    type: 'function',
    name: 'updateOperatorSigningKey',
    inputs: [
      { name: 'blsKey', type: 'bytes', internalType: 'bytes' },
      { name: 'blsSigProof', type: 'bytes', internalType: 'bytes' },
    ],
    outputs: [],
    stateMutability: 'nonpayable',
  },
  // getLatestOperatorSigningKey for BLS: returns bytes (not address)
  {
    type: 'function',
    name: 'getLatestOperatorSigningKey',
    inputs: [{ name: 'operator', type: 'address', internalType: 'address' }],
    outputs: [{ name: '', type: 'bytes', internalType: 'bytes' }],
    stateMutability: 'view',
  },
  // operatorRegistered is the same
  {
    type: 'function',
    name: 'operatorRegistered',
    inputs: [{ name: 'operator', type: 'address', internalType: 'address' }],
    outputs: [{ name: '', type: 'bool', internalType: 'bool' }],
    stateMutability: 'view',
  },
] as const;
```

### Pattern 5: One-Click BLS Registration Flow

**What:** Frontend orchestrates the full BLS key registration with a single button click.
**When to use:** Service detail page for BLS services that are not yet registered.

The flow:
1. Get service signer info (BLS G1 pubkey + HD index) via `getServiceSigner()`
2. Get operator's EVM address (HD index 0) from wallet store
3. Call `blsSignProofOfPossession(hdIndex, operatorAddress)` to get G1 + G2 proof
4. Call contract `updateOperatorSigningKey(blsKey, blsSigProof)` using the operator's wallet (HD index 0)
5. The transaction is sent from the operator's EVM address (same address used for `registerOperator`)

```typescript
async function handleBlsRegistration(
  registryAddress: Address,
  rpcUrl: string,
  chainId: number,
  blsHdIndex: number,
  operatorAddress: Address,
) {
  // 1. Get BLS proof of possession from backend
  const proofResp = await blsSignProofOfPossession(blsHdIndex, operatorAddress);

  // 2. Encode as bytes
  const blsKeyHex = `0x${proofResp.g1_pubkey_hex}` as `0x${string}`;
  const blsProofHex = `0x${proofResp.g2_proof_hex}` as `0x${string}`;

  // 3. Send transaction from operator wallet
  const publicClient = getPublicClient(rpcUrl, chainId);
  const walletClient = await getWalletClient(rpcUrl, chainId);

  const hash = await walletClient.writeContract({
    address: registryAddress,
    abi: BLSPOAStakeRegistryABI,
    functionName: 'updateOperatorSigningKey',
    args: [blsKeyHex, blsProofHex],
  });

  await publicClient.waitForTransactionReceipt({ hash });
}
```

### Anti-Patterns to Avoid

- **BLS crypto in JavaScript:** All BLS key derivation and proof generation MUST stay in the Rust backend. The frontend only passes hex strings around. No JavaScript BLS library.
- **Using ECDSA ABI for BLS contract:** The function names are the same but the parameter types differ. Using the wrong ABI will cause ABI encoding errors or contract reverts. Always use the BLS-specific ABI entries when the service algorithm is BLS.
- **Assuming signing key type from registry type:** The algorithm is determined by the service configuration (in the service JSON), not by the registry contract type. Check `service.workflows[*].submit.aggregator.signature_kind.algorithm` to determine ECDSA vs BLS.
- **Hardcoding HD index:** The BLS HD index comes from `getServiceSigner()` response's `bls12381.hd_index` field. Do not hardcode index 0 -- different services may use different HD indices.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| BLS key derivation | JS BLS library | `cmd_derive_bls_pubkey` Tauri command | Security: private key never leaves Rust |
| BLS proof-of-possession | JS BLS signing | `cmd_bls_sign_proof_of_possession` Tauri command | DST must match contract; blst library is Rust-only |
| Copy-to-clipboard | Custom clipboard logic | `AddressDisplay` component | Already handles copy + truncation |
| Contract writes | Manual ABI encoding | viem `writeContract` | Already used throughout `utils/evm.ts` |
| Registration status check | Custom contract call | viem `readContract` + existing pattern from P2P page | Verified pattern from Phase 10 |
| Algorithm detection | Manual service JSON parsing | `'secp256k1' in signerResponse` discriminated union check | Type-safe at compile time |

**Key insight:** The frontend is an orchestration layer. It calls Tauri commands for crypto operations and viem for contract interactions. No cryptographic computation happens in JavaScript.

## Common Pitfalls

### Pitfall 1: Wrong ABI for BLS Registry Contract

**What goes wrong:** Calling `updateOperatorSigningKey(address, bytes)` (ECDSA ABI) on a BLS POA registry contract, which expects `(bytes, bytes)`. Transaction reverts with "invalid signature" or ABI encoding fails.
**Why it happens:** Both contract versions have the same function name but different parameter types. The existing `POAStakeRegistryABI` only has the ECDSA signature.
**How to avoid:** Add `BLSPOAStakeRegistryABI` entries. Determine which ABI to use based on the service's `signatureAlgorithm` field, NOT the registry contract address.
**Warning signs:** ABI encoding error in viem, or contract reverts with `InvalidBLSKeyLength()` / `InvalidBLSSignatureLength()`.

### Pitfall 2: BLS Proof-of-Possession Digest Mismatch

**What goes wrong:** The G2 proof doesn't verify on-chain because the digest was computed incorrectly.
**Why it happens:** The BLS contract expects `keccak256(abi.encode(operator))` where `operator` is `msg.sender` (the HD index 0 operator address). If the wrong address is used, or ABI encoding differs, the proof fails.
**How to avoid:** The new `cmd_bls_sign_proof_of_possession` command takes `operator_address` as a parameter and uses `alloy_sol_types::SolValue::abi_encode` (same as the existing `chain_ops.rs` ECDSA flow). The operator address MUST be the EVM address at HD index 0 of the signing mnemonic -- the same address that sends the transaction.
**Warning signs:** Contract reverts with `InvalidBLSKeyOwnershipProof()` error.

### Pitfall 3: BLS getLatestOperatorSigningKey Returns bytes, Not address

**What goes wrong:** Reading the BLS contract's `getLatestOperatorSigningKey` with the ECDSA ABI, which expects `address` return type. This returns a truncated or malformed value.
**Why it happens:** The ECDSA version returns `address` (20 bytes), the BLS version returns `bytes` (128 bytes). Using the wrong ABI causes viem to decode incorrectly.
**How to avoid:** Use `BLSPOAStakeRegistryABI` for reading BLS operator signing keys. Compare the returned bytes to determine if registered (non-empty bytes vs empty bytes/zero).
**Warning signs:** Garbled signing key values, incorrect registration status.

### Pitfall 4: signaturePrefix Must Be 'none' for BLS

**What goes wrong:** Service deployed with BLS algorithm but EIP-191 signature prefix. The BLS signing pipeline does not use EIP-191 prefix -- it uses raw digest signing.
**Why it happens:** The default `signaturePrefix` is 'eip191'. If the user selects BLS without the prefix being auto-changed, the service JSON will have an incompatible configuration.
**How to avoid:** When the algorithm selector changes to 'bls12381', auto-set `signaturePrefix` to 'none'. The `SubmitEditor` component should enforce this coupling.
**Warning signs:** BLS service fails to aggregate signatures because prefix mismatch.

### Pitfall 5: Operator EVM Address Required for BLS Registration

**What goes wrong:** The operator tries to register their BLS key but can't determine their EVM operator address (HD index 0).
**Why it happens:** BLS services use a BLS signing key, but the on-chain registration still requires an EVM transaction from the operator's EVM address.
**How to avoid:** The wallet store already tracks derived addresses (HD index 0 is the default). Use `walletStore.derivedAddresses[0]` or `getAddress()` from `useViemClient` to get the operator's EVM address for the registration transaction.
**Warning signs:** "No wallet connected" error when trying to register BLS key.

### Pitfall 6: Detecting Service Algorithm

**What goes wrong:** Trying to determine if a service uses BLS by looking at the registry, but the registry doesn't store the algorithm type.
**Why it happens:** The algorithm is part of the service JSON configuration, not an on-chain attribute. The contract doesn't distinguish ECDSA vs BLS at the registry level -- it's the specific POA middleware deployment type that determines this.
**How to avoid:** Check the service's workflow submit configuration: `service.workflows[wfId].submit.aggregator.signature_kind.algorithm`. If the service is loaded from the WAVS node (via `getServices()`), this field is populated. Alternatively, check the `SignerResponse` type from `getServiceSigner()` -- if it's `bls12381` variant, the service is BLS.
**Warning signs:** Always showing ECDSA registration UI even for BLS services.

## Code Examples

### Detecting BLS Service from Service Object

```typescript
// Source: types/index.ts Service/Workflow/Submit types
function isBLSService(service: Service): boolean {
  return Object.values(service.workflows).some((wf) => {
    if (wf.submit === 'none') return false;
    return wf.submit.aggregator.signature_kind.algorithm === 'bls12381';
  });
}
```

### BLS Registration Status Check (different from ECDSA)

```typescript
// For BLS: getLatestOperatorSigningKey returns bytes (128 bytes when set, empty when not)
// For ECDSA: getLatestOperatorSigningKey returns address (zero address when not set)

async function checkBlsRegistrationStatus(
  publicClient: PublicClient,
  registryAddress: Address,
  operatorAddress: Address,
): Promise<'registered' | 'unregistered' | 'unknown'> {
  try {
    const signingKey = await publicClient.readContract({
      address: registryAddress,
      abi: BLSPOAStakeRegistryABI,
      functionName: 'getLatestOperatorSigningKey',
      args: [operatorAddress],
    });
    // BLS returns bytes - empty or "0x" means unregistered
    const keyHex = signingKey as `0x${string}`;
    return keyHex.length > 2 ? 'registered' : 'unregistered';
  } catch {
    return 'unknown';
  }
}
```

### Updated evm.ts for BLS signing key update

```typescript
// Source: adapted from existing updateSigningKey in evm.ts
export async function updateBlsSigningKey(
  publicClient: PublicClient<Transport, Chain>,
  walletClient: WalletClient<Transport, Chain, HDAccount>,
  registryAddress: Address,
  blsKeyHex: `0x${string}`,
  blsProofHex: `0x${string}`,
): Promise<`0x${string}`> {
  const hash = await walletClient.writeContract({
    address: registryAddress,
    abi: BLSPOAStakeRegistryABI,
    functionName: 'updateOperatorSigningKey',
    args: [blsKeyHex, blsProofHex],
  });
  await publicClient.waitForTransactionReceipt({ hash });
  return hash;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Service builder hardcoded to secp256k1 | `signatureAlgorithm` type widened to union | Phase 9 (2026-03-24) | Builder store ready for BLS selection |
| No BLS key display | `cmd_derive_bls_pubkey` available | Phase 9 (2026-03-24) | Can derive and display BLS pubkey |
| Registration check via ECDSA signing key address | Dual-path: ECDSA uses address, BLS uses bytes | Phase 11 (this phase) | Need BLS-specific ABI for on-chain reads |
| ECDSA-only signing key update | BLS proof-of-possession flow | Phase 11 (this phase) | New Tauri command + BLS ABI entry needed |

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Vite build (TypeScript) + cargo check (Rust) + manual visual |
| Config file | tsconfig.json, Cargo.toml |
| Quick run command | `cd app && npx tsc --noEmit` + `cargo check -p wavs-app` |
| Full suite command | `just app-build-frontend` (Vite build) |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BLS-01 | Algorithm selector visible in submit editor | manual-only | Visual: open service builder, select aggregator, verify ECDSA/BLS dropdown appears | N/A |
| BLS-02 | Post-deploy BLS pubkey display | manual-only | Visual: deploy BLS service, verify G1 pubkey appears with copy button | N/A |
| BLS-03 | One-click BLS key registration | manual-only | Visual: click "Register BLS Key", verify on-chain tx succeeds | N/A |
| BLS-04 | BLS registration status on service detail | manual-only | Visual: navigate to BLS service detail, verify registration badge | N/A |

**Justification for manual-only:** All requirements are UI interactions backed by Tauri commands (already tested via E2E suite). The Rust `bls_signing.rs` has extensive unit tests. Frontend rendering is best verified visually.

### Sampling Rate

- **Per task commit:** `just app-build-frontend` (Vite build) + `cargo check -p wavs-app`
- **Per wave merge:** `just app-dev` visual smoke test
- **Phase gate:** Full visual walkthrough of all 4 success criteria

### Wave 0 Gaps

None -- existing build infrastructure covers all validation needs. The `bls_signing.rs` module already has tests for key derivation and signing.

## Open Questions

1. **How to determine BLS vs ECDSA registry at the contract level**
   - What we know: The algorithm is in the service JSON. But on the service detail page, we may be viewing a registry that has no service registered yet. The `fetchOperators()` function reads `getLatestOperatorSigningKey` with the ECDSA ABI (returns `address`). For BLS registries, this would need to read with the BLS ABI (returns `bytes`).
   - What's unclear: How to reliably detect registry type before a service is registered.
   - Recommendation: For the service detail page, check if a service is loaded for this registry. If yes, check its algorithm. If no service is loaded, fall back to ECDSA behavior (existing). The BLS registration button should only appear when a service with `bls12381` algorithm is associated with the registry. This avoids the detection problem entirely.

2. **HD index for BLS proof-of-possession**
   - What we know: The service signer (`getServiceSigner()`) returns `bls12381.hd_index`. This is the BLS key's HD index. The proof must be signed with this key.
   - What's unclear: Whether the WAVS node must be running to get the HD index, or if the app can infer it (default: 0).
   - Recommendation: Call `getServiceSigner()` when the WAVS node is running. If unavailable, default to HD index 0 (the standard default). The `BlsProofResponse` also returns `g1_pubkey_hex` so we can verify it matches.

3. **operator_address derivation for proof**
   - What we know: The operator address is HD index 0 of the signing mnemonic. This is the same as the wallet's first derived address.
   - What's unclear: Whether to derive the operator address in Rust (from mnemonic) or pass it from the frontend (from walletStore).
   - Recommendation: Pass from frontend -- `walletStore.derivedAddresses[0]` is already available and avoids reimplementing derivation. The Tauri command takes it as a parameter.

## Sources

### Primary (HIGH confidence)

- Codebase: `packages/types/src/contracts/solidity/abi/bls/IPOAStakeRegistry.json` -- BLS POA contract ABI with `updateOperatorSigningKey(bytes blsKey, bytes blsSigProof)` and `getLatestOperatorSigningKey(address) -> bytes`
- Codebase: `packages/utils/src/bls_signing.rs` -- `bls_private_key_from_mnemonic()`, `bls_g1_pubkey_bytes()`, `bls_sign_digest()` functions
- Codebase: `packages/utils/src/test_utils/middleware/evm/middleware_poa_bls.rs` -- BLS middleware test configuration showing `updateOperatorSigningKey(bytes,bytes)` call pattern with cast
- Codebase: `packages/wavs-mcp/src/chain_ops.rs:282-292` -- ECDSA proof pattern: `keccak256(abi.encode(operator))` signed by signing key
- Codebase: `app/src/components/service/SubmitEditor.tsx` -- Current submit editor with dropdowns
- Codebase: `app/src/components/service/ServiceDeploy.tsx` -- Current deploy flow
- Codebase: `app/src/pages/services/ServiceDetailPage.tsx` -- Current service detail page
- Codebase: `app/src/contracts/POAStakeRegistry.ts` -- ECDSA ABI with `updateOperatorSigningKey(address, bytes)`
- Codebase: `app/src/utils/evm.ts` -- Existing `updateSigningKey()` and `createSigningKeySignature()` ECDSA functions
- Codebase: `app/src/stores/serviceBuilderStore.ts` -- `SubmitDraft.signatureAlgorithm` already typed as `SignatureAlgorithm`
- Codebase: `app/src-tauri/src/commands.rs:1290-1303` -- Existing `cmd_derive_bls_pubkey` implementation

### Secondary (MEDIUM confidence)

- `.planning/STATE.md` -- Research flag: "Phase 11 `cmd_derive_bls_pubkey` proof-of-possession encoding must match `IPOAStakeRegistry.updateOperatorSigningKey` contract expectations"
- Phase 9 RESEARCH.md -- Foundation types and command architecture
- Phase 10 RESEARCH.md -- P2P page registration check patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, zero new dependencies
- Architecture: HIGH -- direct pattern reuse from Phase 10 (registration), Phase 9 (commands), existing service builder. BLS contract ABI verified against actual JSON source.
- Pitfalls: HIGH -- verified BLS contract ABI differences, proof-of-possession flow, and DST requirements against actual Rust source code. The ECDSA vs BLS ABI mismatch is the critical discovery.
- New Rust command: HIGH -- `bls_sign_digest()` already exists and is tested. The new command just wires it to Tauri with the correct digest computation.

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable -- no external dependency changes expected)
