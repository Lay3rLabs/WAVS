# Deployment Flow

Deploy a new WAVS service with an on-chain ServiceManager contract.

---

## Checklist

- [ ] **Step 1** — `wavs:wavs_get_health` — Confirm all chain RPC endpoints are healthy before spending gas.
- [ ] **Step 2** — Deploy a ServiceManager contract (pass `rpc_url`, e.g. `"http://localhost:8545"`):
  - **SimpleServiceManager** (lightweight PoA): `wavs:wavs_deploy_service_manager` — returns `address`
  - **POAStakeRegistry** (full middleware): `wavs:wavs_deploy_poa_service_manager` — returns proxy `address`; requires Docker
- [ ] **Step 3 (POA only)** — `wavs:wavs_register_operator` — Register the node's signing key as an operator on the POAStakeRegistry.
- [ ] **Step 4** — `wavs:wavs_upload_component` — Upload the compiled `.wasm`; save the returned digest (`sha256:...`).
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

- Uses `chain_write_credential` as the contract owner to call `registerOperator(signingKeyAddr, weight)`
- Uses `signing_mnemonic` at HD index 0 for the operator to call `updateOperatorSigningKey(signingKeyAddr, sig)`
- Default weight is `100` if not specified
- The signing key address at index 0 is derived from `signing_mnemonic` (Anvil default: `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`)

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

## Dev-Only Deployment (No On-Chain Contract)

For local testing without deploying a contract, use `wavs_deploy_dev_service` instead of steps 2–7:

```
wavs:wavs_deploy_dev_service  {service_json}
```

Requires `dev_endpoints_enabled = true` in `wavs.toml`. The service is registered directly from the JSON without an on-chain ServiceManager.
