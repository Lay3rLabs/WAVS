# Update Service Flow

Swap in a new component version for a deployed WAVS service.

---

## Checklist

- [ ] **Step 1** — `wavs:wavs_list_services` — Find the `service_id` and the service manager address.
- [ ] **Step 2** — `wavs:wavs_get_service` — Inspect the current workflow, component digest, and trigger config.
- [ ] **Step 3** — Build and upload new component:
  - `wavs:wavs_build_component` — Compile; fix errors; repeat until exit code 0.
  - `wavs:wavs_upload_component` — Upload new `.wasm`; save the new digest.
- [ ] **Step 4** — `wavs:wavs_save_service` — Save updated service definition with the new component digest; get a new URI.
- [ ] **Step 5** — `wavs:wavs_set_service_uri` — Update the URI on-chain.
- [ ] **Step 6** — `wavs:wavs_deploy_service` — Re-register the updated service with the WAVS node.
- [ ] **Step 7** — `wavs:wavs_simulate_trigger` — Verify the new behavior.

---

## Pausing and Resuming a Service

Pause and resume are done by updating the service definition's `status` field and setting the new URI on-chain. There are no separate pause/resume MCP tools.

**To pause:**
1. Get the current service definition (from step 2 above or `wavs_get_service`)
2. Change the `status` field from `"active"` to `"paused"`
3. `wavs:wavs_save_service` — Save the updated definition; get a new URI
4. `wavs:wavs_set_service_uri` — Point the on-chain contract to the new URI
5. `wavs:wavs_deploy_service` — Re-register so the node picks up the change

**To resume:**
1. Change the `status` field from `"paused"` back to `"active"`
2. `wavs:wavs_save_service` → `wavs:wavs_set_service_uri` → `wavs:wavs_deploy_service`

---

## Pause vs Delete

| | Pause (via service URI update) | Delete |
|---|---|---|
| How | Set `status: "paused"` in service JSON, save, update URI on-chain, re-deploy | `wavs_delete_service` |
| Effect | Stops trigger execution; service stays registered | Removes service from WAVS node entirely |
| Recovery | Set `status: "active"`, save, update URI, re-deploy | Must re-deploy from scratch |
| Use for updates | Yes — pause while swapping, resume after | No — disruptive |
| Use when decommissioning | No | Yes |

---

## Checking Service Status

```
wavs:wavs_get_service  {chain, address}
```

Look for `status` in the response: `active` | `paused`.

A paused service will not fire triggers but keeps its registration. The WAVS node checks this before dispatching.

---

## Rollback

If the new component has issues:

1. Set `status` to `"paused"` in the service definition, save, update URI, and re-deploy to stop the broken version
2. Re-upload the previous `.wasm` (keep old digests saved)
3. `wavs:wavs_save_service` with the old digest → new URI
4. `wavs:wavs_set_service_uri` — restore old URI on-chain
5. `wavs:wavs_deploy_service` — re-register
6. Set `status` back to `"active"`, save, update URI, and re-deploy to resume

---

## Updating Only Configuration (No New Component)

If you only need to change the service config (e.g. trigger schedule, config vars) without a new WASM binary:

- Skip step 3
- In step 4, save a new service definition with the **same** component digest but updated config/triggers
- Continue from step 5

---

## Service ID

The `service_id` is a 64-char hex string derived from the ServiceManager address. You can find it in `wavs_list_services` output. It's required for `wavs_simulate_trigger` and `wavs_query_kv`.
