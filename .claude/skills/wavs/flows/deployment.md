# Deployment Flow

Deploy a new WAVS service with an on-chain ServiceManager contract.

---

## Checklist

- [ ] **Step 0** — Check existing state (always do this first):
  - Call `wavs:wavs_list_services` and `wavs:wavs_get_node_info` before anything else.
  - | What you find | What to do |
    |---------------|-----------|
    | No services registered | Continue with Step 1 below |
    | Service already exists for this contract | Go to [update-service.md](update-service.md) instead |
    | Service exists but is paused | Resume it, or go to update-service.md to update it |
- [ ] **Step 1** — `wavs:wavs_get_health` — Confirm all chain RPC endpoints are healthy before spending gas.
- [ ] **Step 2** — Deploy a ServiceManager contract (pass `rpc_url`, e.g. `"http://localhost:8545"`):
  - **SimpleServiceManager** (lightweight PoA): `wavs:wavs_deploy_service_manager` — returns `address`
  - **POAStakeRegistry** (full middleware): `wavs:wavs_deploy_poa_service_manager` — returns proxy `address`; requires Docker
- [ ] **Step 3** — `wavs:wavs_upload_component` — Upload the compiled `.wasm`; save the returned digest (raw 64-char hex, no `sha256:` prefix).
- [ ] **Step 4** — `wavs:wavs_save_service` — Save the service definition JSON; get back a URI.
- [ ] **Step 5** — `wavs:wavs_set_service_uri` — Call `setServiceURI` on-chain with the URI from step 4.
- [ ] **Step 6** — `wavs:wavs_deploy_service` — Register the service with the WAVS node (reads definition from chain).
  - **Verify the node picked up the service:** Call `wavs:wavs_get_service(chain, address)` and confirm `status: active`. Note: `wavs_deploy_service` returning success only means the node fetched and stored the config from the on-chain URI. Confirm active status with `wavs_get_service`.
- [ ] **Step 7 (POA only)** — `wavs:wavs_register_operator` — Register the node's signing key on the POAStakeRegistry. **Must be called AFTER `wavs_deploy_service`** so the node has assigned a service-specific HD-derived signing key to register on-chain.
- [ ] **Step 8** — `wavs:wavs_simulate_trigger` — Smoke test.
- [ ] **Step 9** — `wavs:wavs_list_services` — Confirm the service appears with `status: active`.

---

## SimpleServiceManager vs POAStakeRegistry

| | SimpleServiceManager | POAStakeRegistry |
|---|---|---|
| Tool | `wavs_deploy_service_manager` | `wavs_deploy_poa_service_manager` |
| Operator registration | Not required | Required (`wavs_register_operator`) |
| Docker required | No | Yes (`ghcr.io/lay3rlabs/poa-middleware:1.0.1`) |
| Use case | Quick dev/testing | Production with operator weighting |
| `mcp_chain_credential` | Required | Required |

---

## wavs_register_operator Notes

**Call this AFTER `wavs_deploy_service`** — the tool queries the WAVS node for the service-specific HD-derived signing key (the key the node actually uses to sign envelopes) and registers that address on-chain. If the service is not deployed yet the tool will return an error.

`wavs_register_operator` sends **two sequential on-chain transactions** from two different signers:

1. `registerOperator(operatorAddr, weight)` — sent by `mcp_chain_credential` (registry owner); registers HD index 0 of `signing_mnemonic` as the stable operator identity
2. `updateOperatorSigningKey(serviceSigningKeyAddr, sig)` — sent by `signing_mnemonic` at HD index 0 (operator); sets the signing key to the service-specific HD index the node actually uses

Both must succeed for the node to be fully registered.

Default weight is `100` if not specified.

**Idempotent:** If `registerOperator` reverts with `AlreadyRegistered` (e.g. after a previous partial failure), the tool skips that step and still proceeds to call `updateOperatorSigningKey`. This means it's safe to retry — the signing key will always be set even if registration already happened.

**Revert decoding:** Known 4-byte selectors are decoded automatically. `InvalidSignature (0x8baa579f)` means the wrong signing key was used; the error message includes the HD index and address that were attempted so you can diagnose mismatches.

---

## HD Index Assignment

Each service gets a unique HD-derived signing key. The index is assigned when the service is registered with the node (`wavs_deploy_service` or `wavs_deploy_dev_service`).

**Key facts:**
- The counter starts at `1` by default (not `0`) — both in-memory and in `~/.wavs/data/service_registry.json`
- HD index 0 is reserved as the stable **operator identity** (used for `registerOperator`)
- After deleting a service, the counter does NOT reset — the deleted service's index is never reused
- **Manually editing `service_registry.json` to reset `next_hd_index: 0` is unreliable** unless the node is also restarted, because the in-memory counter is independent of the file
- Use `wavs_get_service_signer` to inspect the actual HD index for any deployed service

**Diagnosing `InvalidSignature` / `SignerNotRegistered` reverts:**
1. Run `wavs_get_service_signer` → note the HD index (e.g. `5`)
2. Run `wavs_get_signing_address(hd_index=5)` → verify the address
3. Compare with what's registered on POAStakeRegistry via `getLatestSigningKeyForOperator`
4. Re-run `wavs_register_operator` to update the on-chain mapping

---

## Service Definition JSON

Pass to `wavs_save_service` or `wavs_deploy_dev_service`:

```json
{
  "name": "my-service",
  "status": "active",
  "manager": {
    "evm": {
      "chain": "evm:31337",
      "address": "0xServiceManagerAddress..."
    }
  },
  "workflows": {
    "default": {
      "trigger": {
        "cron": {
          "schedule": "* * * * * * *",
          "start_time": null,
          "end_time": null
        }
      },
      "component": {
        "source": {
          "digest": "<64-char hex from wavs_upload_component>"
        },
        "permissions": {
          "file_system": false,
          "allowed_http_hosts": "none",
          "raw_sockets": false,
          "dns_resolution": false
        },
        "fuel_limit": null,
        "time_limit_seconds": null,
        "config": {},
        "env_keys": []
      },
      "submit": "none"
    }
  }
}
```

See [`reference/service-json.md`](../reference/service-json.md) for all trigger types and full format details.

---

## ServiceManager JSON Format

Used by `wavs_deploy_service`, `wavs_set_service_uri`, `wavs_register_operator`, and service lifecycle tools:

```json
// EVM
{"evm": {"chain": "evm:31337", "address": "0xAbCd1234..."}}

// Cosmos
{"cosmos": {"chain": "cosmos:mychain", "address": "cosmos1abc..."}}
```

---

## Flow Notes

**Why 3 steps for production?** `wavs_save_service` (store config on node) →
`wavs_set_service_uri` (write URI on-chain) → `wavs_deploy_service` (node fetches config
from chain URI and activates). These are separate because the URI can also be set by other
tooling (e.g. `just deploy-service`). For dev/testing, `wavs_deploy_dev_service` does all
three in one call.

---

## Known Quirks

| Tool | Symptom | Actual Outcome | Action |
|------|---------|----------------|--------|
| `wavs_deploy_service` | Returns empty body | Service was registered successfully | Response is now shown as "Service registered successfully." |

---

## Dev-Only Deployment (No On-Chain Contract)

For local testing without deploying a contract, use `wavs_deploy_dev_service` instead of steps 2–7:

```
wavs:wavs_deploy_dev_service  {service_json}
```

Requires `dev_endpoints_enabled = true` in `wavs.toml`. The service is registered directly from the JSON without an on-chain ServiceManager.

---

## Complete Example (PoA + manual trigger)

This walkthrough uses `echo_data.wasm` on a local Anvil chain (`evm:31337`). All values shown are real.

### Step 1 — Health check
```
wavs_get_health()
→ {"evm:31337": "ok"}
```

### Step 2 — Deploy POAStakeRegistry
```
wavs_deploy_poa_service_manager(rpc_url="http://localhost:8545")
→ POAStakeRegistry deployed.
  Address (use as service manager): 0x8a791620dd6260079bf849dc5567adc3f2fdc318
```

### Step 3 — Build and upload component
```
wavs_build_component(dir="/path/to/my-component")
→ Exit code: 0
  ...
  Output WASM files:
    /path/to/my-component/target/wasm32-wasip1/release/my_component.wasm

wavs_upload_component(file_path="/path/to/my-component/target/wasm32-wasip1/release/my_component.wasm")
→ Component uploaded.
  Digest: f0b42a5171c9dcd75eac41c8ce2c4e7882d304c885266d8ac7b70af996b9a420
```

Note: `examples/build/components/echo_data.wasm` is a pre-built component you can use directly.

### Step 4 — Save service definition

```
wavs_save_service(service_json='{
  "name": "echo-manual",
  "status": "active",
  "manager": {
    "evm": {"chain": "evm:31337", "address": "0x8a791620dd6260079bf849dc5567adc3f2fdc318"}
  },
  "workflows": {
    "default": {
      "trigger": "manual",
      "component": {
        "source": {"digest": "f0b42a5171c9dcd75eac41c8ce2c4e7882d304c885266d8ac7b70af996b9a420"},
        "permissions": {"file_system": false, "allowed_http_hosts": "none", "raw_sockets": false, "dns_resolution": false},
        "fuel_limit": null,
        "time_limit_seconds": null,
        "config": {},
        "env_keys": []
      },
      "submit": "none"
    }
  }
}')
→ Service saved.
  URI: http://127.0.0.1:8041/dev/services/a3f5f24b9e12...
```

### Step 5 — Set URI on-chain
```
wavs_set_service_uri(
  service_manager_json={"evm":{"chain":"evm:31337","address":"0x8a791620dd6260079bf849dc5567adc3f2fdc318"}},
  uri="http://127.0.0.1:8041/dev/services/a3f5f24b9e12...",
  rpc_url="http://localhost:8545"
)
→ Service URI updated on-chain successfully
```

### Step 6 — Register with WAVS node
```
wavs_deploy_service(
  service_manager_json={"evm":{"chain":"evm:31337","address":"0x8a791620dd6260079bf849dc5567adc3f2fdc318"}}
)
→ Service registered successfully.
```

### Step 7 — Register operator (POA only)
```
wavs_register_operator(
  service_manager_json={"evm":{"chain":"evm:31337","address":"0x8a791620dd6260079bf849dc5567adc3f2fdc318"}},
  rpc_url="http://localhost:8545"
)
→ Operator registered.
  Signing key (HD index 1): 0xabc123...
  Register tx: 0xabc123...
  Signing key tx: 0xdef456...
```

### Step 8 — Smoke test
```
wavs_simulate_trigger(
  service_id="b3f4249f...",   ← from wavs_list_services
  workflow_id="default",
  trigger_json={"manual": null},
  data_json={"Raw": [72, 101, 108, 108, 111]}   ← "Hello" as bytes
)
→ Trigger simulated successfully
```

### Step 9 — Confirm active
```
wavs_list_services()
→ {"service_ids": ["b3f4249f..."], "services": {...}}
```
