# Service JSON Reference

JSON formats for `wavs_save_service`, `wavs_deploy_dev_service`, and `wavs_simulate_trigger`.

---

## ServiceManager JSON

Used by: `wavs_deploy_service`, `wavs_delete_service`, `wavs_set_service_uri`, `wavs_register_operator`

```json
// EVM
{
  "evm": {
    "chain": "evm:31337",
    "address": "0xAbCd1234..."
  }
}

// Cosmos
{
  "cosmos": {
    "chain": "cosmos:mychain",
    "address": "cosmos1abc..."
  }
}
```

---

## Full Service Definition

Used by: `wavs_save_service`, `wavs_deploy_dev_service`

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
      "trigger": { /* see Trigger Types below */ },
      "component": {
        "source": {
          "digest": "f0b42a5171c9dcd75eac41c8ce2c4e7882d304c885266d8ac7b70af996b9a420"
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

- `name`: human-readable service name
- `status`: `"active"` or `"paused"`
- `manager`: ServiceManager contract (EVM or Cosmos)
- `workflows`: map of `workflow_id` → workflow definition; `workflow_id` is lowercase alphanumeric 3–36 chars
- `component.source.digest`: raw 64-char hex string returned by `wavs_upload_component` (no `sha256:` prefix)
- `submit`: `"none"` to discard results, or `{"aggregator": {...}}` for on-chain submission

Multiple workflows in one service:
```json
{
  "workflows": {
    "price-feed": { "trigger": {...}, "component": {...}, "submit": "none" },
    "heartbeat":  { "trigger": {...}, "component": {...}, "submit": "none" }
  }
}
```

---

## Trigger Types

### Cron
```json
{
  "cron": {
    "schedule": "* * * * * * *",
    "start_time": null,
    "end_time": null
  }
}
```
`schedule` is a 7-field cron expression: `sec min hour dom month dow year` (e.g. `* * * * * * *` = every second). `start_time` and `end_time` are optional unix timestamps.

### Block Interval
```json
{
  "block_interval": {
    "chain": "evm:31337",
    "interval": 10
  }
}
```
Fires every `interval` blocks.

### EVM Contract Event
```json
{
  "evm_contract_event": {
    "chain": "evm:31337",
    "address": "0xTriggerContractAddress...",
    "event_hash": "0x<32-byte-keccak-of-event-signature>"
  }
}
```
`event_hash` is the Keccak-256 hash of the full event signature string, e.g. `keccak256("Transfer(address,address,uint256)")`.

### Cosmos Contract Event
```json
{
  "cosmos_contract_event": {
    "chain": "cosmos:mychain",
    "address": "cosmos1contract...",
    "event_type": "wasm-my-event"
  }
}
```

### Manual
```json
"manual"
```
Unit variant — use the bare string `"manual"` in the service definition `trigger` field. Only fires when explicitly triggered via `wavs_simulate_trigger`.

---

## Aggregator Submit Pattern

Use `"submit": "none"` to discard component output (suitable for dev/testing or side-effect-only components).

Use `{"aggregator": {...}}` when you need the component's output **submitted on-chain** to a receiver contract after consensus.

### When you need aggregator submit

- The component produces a result that must be written back to a smart contract
- Multiple operators run the component and results are aggregated before submission
- The receiver contract has a method like `addPayload(bytes)` that accepts the aggregated output

### Full aggregator submit field

```json
"submit": {
  "aggregator": {
    "component": {
      "source": {"digest": "<digest of simple-aggregator.wasm>"},
      "permissions": {
        "file_system": false,
        "allowed_http_hosts": "none",
        "raw_sockets": false,
        "dns_resolution": false
      },
      "fuel_limit": null,
      "time_limit_seconds": null,
      "config": {
        "chain": "evm:31337",
        "service_handler": "0xReceiverContractAddress"
      },
      "env_keys": []
    },
    "signature_kind": {
      "algorithm": "secp256k1",
      "prefix": "eip191"
    }
  }
}
```

The receiver contract address goes in `component.config["service_handler"]` and the chain key in `component.config["chain"]`. There is **no** top-level `contract`, `quorum_percent`, or `allowed_operators` field in the aggregator submit type.

### Critical: upload simple-aggregator.wasm as a second component

When using aggregator submit, you must call `wavs_upload_component` **twice** — once for your main component, once for `simple-aggregator.wasm`:

```
wavs_upload_component(file_path="examples/build/components/simple-aggregator.wasm")
→ Digest: <aggregator-digest>
```

Use `<aggregator-digest>` in `submit.aggregator.component.source.digest`.

### Pipeline

```
[Trigger fires]
    → [Your component runs, produces output bytes]
        → [simple-aggregator.wasm collects results from operators]
            → [Quorum reached → calls contract.method on-chain]
```

---

## SimulateTrigger Examples

`wavs_simulate_trigger` requires: `service_id`, `workflow_id`, `trigger_json`, `data_json`

### Manual Trigger
```json
trigger_json: {"manual": null}
data_json:    {"Raw": [104, 101, 108, 108, 111]}
```
`Raw` value is a JSON array of byte values.

### Cron Trigger
```json
trigger_json: {"cron": {"schedule": "* * * * * * *", "start_time": null, "end_time": null}}
data_json:    {"Cron": {"trigger_time": 1700000000}}
```

### Block Interval Trigger
```json
trigger_json: {"block_interval": {"chain": "evm:31337", "interval": 10}}
data_json:    {"BlockInterval": {"block_height": 42}}
```

### EVM Contract Event
```json
trigger_json: {
  "evm_contract_event": {
    "chain": "evm:31337",
    "address": "0xTriggerContract...",
    "event_hash": "0x<32-byte-keccak>"
  }
}
data_json: {
  "EvmContractEvent": {
    "log": {
      "address": "0xTriggerContract...",
      "data": "0x<abi-encoded-event-data>",
      "topics": ["0x<event-hash>", "0x<indexed-param-1>"]
    }
  }
}
```

---

## Service ID

The `service_id` is a 64-character hex string derived from the ServiceManager contract address. Find it in `wavs_list_services` output. Required for `wavs_simulate_trigger` and `wavs_query_kv`.

---

## Config Variables in Service Definition

To pass configuration to a component via `host::config_var("key")`, include a `config` map in the workflow:

```json
{
  "workflows": {
    "default": {
      "trigger": {...},
      "component": {
        "source": {"digest": "<64-char hex from wavs_upload_component>"},
        "config": {
          "api-url": "https://api.example.com",
          "threshold": "100"
        }
      },
      "submit": {...}
    }
  }
}
```

All config values are strings. Parse numbers with `.parse::<u64>()` etc. in the component.
