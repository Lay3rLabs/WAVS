# Deployment Flow

Deploy a new WAVS service with an on-chain ServiceManager contract.

---

## Checklist

- [ ] **Step 1** — `wavs:wavs_get_health` — Confirm all chain RPC endpoints are healthy before spending gas.
- [ ] **Step 2** — Deploy a ServiceManager contract (pass `rpc_url`, e.g. `"http://localhost:8545"`):
  - **SimpleServiceManager** (lightweight PoA): `wavs:wavs_deploy_service_manager` — returns `address`
  - **POAStakeRegistry** (full middleware): `wavs:wavs_deploy_poa_service_manager` — returns proxy `address`; requires Docker
- [ ] **Step 3 (POA only)** — `wavs:wavs_register_operator` — Register the node's signing key as an operator on the POAStakeRegistry.
- [ ] **Step 4** — `wavs:wavs_upload_component` — Upload the compiled `.wasm`; save the returned digest (raw 64-char hex, no `sha256:` prefix).
- [ ] **Step 5** — `wavs:wavs_save_service` — Save the service definition JSON; get back a URI.
- [ ] **Step 6** — `wavs:wavs_set_service_uri` — Call `setServiceURI` on-chain with the URI from step 5.
- [ ] **Step 7** — `wavs:wavs_deploy_service` — Register the service with the WAVS node (reads definition from chain).
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
| `chain_write_credential` | Required | Required |

---

## wavs_register_operator Notes

`wavs_register_operator` sends **two sequential on-chain transactions** from two different signers:

1. `registerOperator(signingKeyAddr, weight)` — sent by `chain_write_credential` (registry owner)
2. `updateOperatorSigningKey(signingKeyAddr, sig)` — sent by `signing_mnemonic` at HD index 0 (node operator)

Both must succeed for the node to be fully registered.

Default weight is `100` if not specified.

**Partial-failure quirk:** If the tool returns a "nonce too low" RPC error, `registerOperator` likely succeeded but `updateOperatorSigningKey` was NOT called. Retrying will cause `registerOperator` to revert with `AlreadyRegistered` — and the function will error before reaching `updateOperatorSigningKey`. This means the signing key is never set and the node will fail to submit results on-chain (though `submit: "none"` services will still appear to work).

**Workaround if this happens:** The tool must be fixed to skip past an `AlreadyRegistered` revert and still call `updateOperatorSigningKey`. Until then, if you hit this, redeploy a fresh POAStakeRegistry to get a clean slate, or call `updateOperatorSigningKey` directly on-chain.

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
          "schedule": "* * * * *",
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

## Known Quirks

| Tool | Symptom | Actual Outcome | Action |
|------|---------|----------------|--------|
| `wavs_register_operator` | Returns "nonce too low" | `registerOperator` tx succeeded; `updateOperatorSigningKey` was NOT called | Signing key unset — see notes above |
| `wavs_deploy_service` | Returns EOF/empty-body error | Service was registered successfully | Verify with `wavs_list_services` |

---

## Dev-Only Deployment (No On-Chain Contract)

For local testing without deploying a contract, use `wavs_deploy_dev_service` instead of steps 2–7:

```
wavs:wavs_deploy_dev_service  {service_json}
```

Requires `dev_endpoints_enabled = true` in `wavs.toml`. The service is registered directly from the JSON without an on-chain ServiceManager.
