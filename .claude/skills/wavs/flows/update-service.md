# Update Service Flow

Swap in a new component version for a deployed WAVS service.

---

## Checklist

- [ ] **Step 1** — `wavs:wavs_list_services` — Find the `service_id` and the service manager address.
- [ ] **Step 2** — `wavs:wavs_get_service` — Inspect the current workflow, component digest, and trigger config.
- [ ] **Step 3** — `wavs:wavs_pause_service` — Halt trigger execution while you swap the component.
- [ ] **Step 4** — Build and upload new component:
  - `wavs:wavs_build_component` — Compile; fix errors; repeat until exit code 0.
  - `wavs:wavs_upload_component` — Upload new `.wasm`; save the new digest.
- [ ] **Step 5** — `wavs:wavs_save_service` — Save updated service definition with the new component digest; get a new URI.
- [ ] **Step 6** — `wavs:wavs_set_service_uri` — Update the URI on-chain.
- [ ] **Step 7** — `wavs:wavs_deploy_service` — Re-register the updated service with the WAVS node.
- [ ] **Step 8** — `wavs:wavs_simulate_trigger` — Verify the new behavior.
- [ ] **Step 9** — `wavs:wavs_resume_service` — Re-enable if still paused after the update.

---

## Pause vs Delete

| | Pause | Delete |
|---|---|---|
| Tool | `wavs_pause_service` | `wavs_delete_service` |
| Effect | Stops trigger execution; service stays registered | Removes service from WAVS node entirely |
| Recovery | `wavs_resume_service` | Must re-deploy from scratch |
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

1. `wavs:wavs_pause_service` — stop the broken version
2. Re-upload the previous `.wasm` (keep old digests saved)
3. `wavs:wavs_save_service` with the old digest → new URI
4. `wavs:wavs_set_service_uri` — restore old URI on-chain
5. `wavs:wavs_deploy_service` — re-register
6. `wavs:wavs_resume_service`

---

## Updating Only Configuration (No New Component)

If you only need to change the service config (e.g. trigger schedule, config vars) without a new WASM binary:

- Skip steps 3–4
- In step 5, save a new service definition with the **same** component digest but updated config/triggers
- Continue from step 6

---

## Service ID

The `service_id` is a 64-char hex string derived from the ServiceManager address. You can find it in `wavs_list_services` output. It's required for `wavs_simulate_trigger` and `wavs_query_kv`.
