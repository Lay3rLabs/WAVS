# v3.0 Agent Composition — QA Testing Plan

## Status

All crates compile. ≥29 engine tests pass. Three v3.0 example components
(`utility_service`, `multi_step_agent`, `composition_agent`) build to WASM.
Tauri app surfaces the new fields (`max_continuation_steps`,
`allowed_service_calls`, `allowed_callers`) in the editor and viewer.

This plan walks a tester through validating v3.0 end-to-end **through the
desktop app**, not just engine unit tests.

---

## 1. Quick start (app-driven)

```bash
# Terminal 1 — start the desktop app (also starts the embedded WAVS node on :8041)
just app-dev

# Terminal 2 — deploy all three v3.0 components and fire triggers
bash scripts/test-agent-deploy.sh
```

The script targets `http://127.0.0.1:8041` by default (the app's embedded
node). Pass a URL to override (e.g. `http://127.0.0.1:8000` for a
standalone `just start-wavs-dev` node). Pass `--no-trigger` to deploy only.

Expected output (abridged):

```
Node is up
→ Uploading utility-service component...
  Uploaded: <digest>...
→ Deploying utility-service service...
  Deployed: <hash> (manager.address=0x...01)
... (multi-step-agent, composition-agent) ...
--- Firing triggers ---
→ Triggering utility-service (service_id=...)...
  Trigger response: { ... }
→ Triggering multi-step-agent ...
→ Triggering composition-agent ...
```

If the script reports `FAIL: Cannot reach .../health`, the app isn't
running yet — wait for the Tauri window to load, then retry.

---

## 2. What the script does

`scripts/test-agent-deploy.sh` does for each component:

1. `POST /dev/components` with the WASM bytes → receives a content digest.
2. Reads `examples/components/<name>/service.json`, patches in the
   uploaded digest **and** a unique `manager.evm.address`
   (`0x...01` / `02` / `03`). The unique address is required because
   `ServiceId = sha256(b"evm" || "evm:31337" || address_bytes)` — if all
   three kept the example file's `0x0...0` they'd collide on the same
   service id and clobber each other. For composition-agent it also
   injects `component.config.callee_service_id = <utility-service-id>`
   so the agent knows which service to RPC into (the component reads
   this via `host::config_var("callee_service_id")` —
   `examples/components/composition-agent/src/lib.rs:49`).
3. `POST /dev/services` → receives a `ServiceDigest` hash.
4. `POST /dev/services/<hash>` → activates the service on the node.

Then for each service (unless `--no-trigger`):

5. Computes the `ServiceId` locally (sha256 of the patched manager).
6. `POST /dev/triggers` with a `SimulatedTriggerRequest` carrying
   `trigger: "manual"`, `data: { Raw: [...] }`, `wait_for_completion: false`.
   The trigger is fire-and-forget — the tester observes the result in the
   app's run history (see section 3 below). `wait_for_completion: true`
   is **not** used here because the server-side wait polls the submission
   counter (`packages/wavs/src/http/handlers/debug.rs:69-87`), and these
   services use `submit: "none"`, so the counter never advances and the
   request hangs.

Triggers fire in order: `utility-service` → `multi-step-agent` →
`composition-agent`. Composition fires last so the callee
(`utility-service`) is already live to receive the RPC.

---

## 3. App UI checks

After the script finishes, the app should reflect everything that
happened on the node. Open the Services page and verify each:

| Service | What to look for |
|---------|------------------|
| `utility-service` | Workflow viewer's permissions tab shows `allowed_callers: "all"`. `max_continuation_steps: null`. |
| `multi-step-agent` | Permissions tab shows `max_continuation_steps: 5`. Default `allowed_service_calls` (none). |
| `composition-agent` | Permissions tab shows `allowed_service_calls: "all"` and `max_continuation_steps: 5`. |

Then deploy a fresh service via the Service Builder wizard (Step 2 →
component editor) and confirm the new editor surfaces these fields:

- `Allowed Service Calls` dropdown (none / all / specific) and the
  per-target service-id editor when "specific" is picked.
- Advanced section: `Max Continuation Steps` numeric input.
- Advanced section: `Allowed Callers` dropdown + specific-caller editor.

---

## 4. Edge cases

| Scenario | Expected | How to exercise |
|----------|----------|-----------------|
| Agent returns Continue 100x | Engine terminates at step 10 (default) with `ContinuationLimit` | Edit a copy of the multi-step-agent to never return Done; deploy via the script with `--no-trigger`, then trigger via the app or curl. |
| `max_continuation_steps: 3` | Terminates at step 3 | Set the field in the component editor when deploying via the app. |
| Service A → B → A | Cycle detected, rejected | Deploy two utility-services with `allowed_service_calls: "all"` that each call the other. |
| 6-deep call chain | Depth cap (5) hits | Chain six utility-service deploys. |
| Caller `allowed_service_calls: "none"` | RPC rejected before dispatch | Edit `composition-agent`'s service.json before deploying. |
| Callee `allowed_callers` excludes the caller's id | RPC rejected | Set `allowed_callers: { Specific: ["<other-id>"] }` on utility-service. |
| service.json missing the new fields | Deserializes with defaults (back-compat) | Deploy a legacy-shape service.json via the script. |
| Trigger composition-agent | utility-service workflow run also appears in the app | Run the script default; confirm both services show a recent run. |

---

## 5. Automated tests (regression)

Run before shipping to confirm nothing in v3.0 regressed the engine:

```bash
cargo test -p wavs-engine                       # full engine suite
cargo test -p wavs-engine --test continuation       # ContinuationLimit, KV key fmt
cargo test -p wavs-engine --test continuation_e2e   # 4-step continuation through real WASM
cargo test -p wavs-engine --test rpc                # RPC error variants, cycle detection
cargo test -p wavs-engine --test rpc_e2e            # composition → utility, permission denials
cargo test -p wavs-engine --test basic              # legacy components still work
cargo test -p wavs-engine --test aggregator         # legacy aggregator path
cargo test -p wavs-types                            # serde defaults, back-compat
```

These exercise real WASM through the engine — not mocks.

---

## 6. WASM component inspection (optional)

```bash
ls -la examples/build/components/{multi_step_agent,composition_agent,utility_service}.wasm

# WIT exports (agent components export both `run` and `run-agent`)
wasm-tools component wit examples/build/components/multi_step_agent.wasm
```

---

## 7. Known gaps in the app

Things the script handles that the app does not (yet) expose:

- **No "Send Manual Trigger" button on ServiceDetailPage.** Testers must
  fire triggers via the script or `curl` against `/dev/triggers`. A v3.1
  candidate would be a Tauri command + button that POSTs a
  `SimulatedTriggerRequest` for the currently-open service.
- **Triggers stay "pending" in the UI for `submit: "none"` services.**
  All three v3.0 examples have `submit: "none"`, so the submission
  subsystem never reports completion and the app's run-status display
  doesn't tick over to "done". The actual engine result lands in
  the WAVS node logs (look for `subsystems::engine` lines, or for
  composition-agent specifically, `rpc_caller` lines showing the
  call_service into utility-service). This is purely a UI status thing
  — the runs themselves complete fine.
- **No live continuation-step counter** in the workflow viewer — the
  multi-step nature is invisible mid-run; only the final result is shown.
- **No per-call RPC timeline** — when composition-agent calls
  utility-service the relationship isn't visualised, just the two runs.

The Rust `dev-tool` (`packages/dev-tool/`) also still hardcodes the echo
service for `deploy-service` and `send-triggers`. Adding `--component`,
`--config`, `--service-id`, `--workflow-id`, and `--data` flags would
give a Rust path to the same flow the script provides; for now the
script is the sanctioned QA path.

---

## 8. What's new in the codebase

| File | Change |
|------|--------|
| `wit-definitions/operator/wit/operator.wit` | `step-result` variant, `agent` interface, `call-service` import |
| `packages/types/src/service.rs` | `AllowedServiceCalls`, `AllowedCallers`, `max_continuation_steps` |
| `packages/engine/src/worlds/operator/execute.rs` | `execute_agent()` continuation loop, `execute_legacy()` fallback |
| `packages/engine/src/rpc.rs` | `RpcCaller` trait |
| `packages/engine/src/bindings/operator/host.rs` | Async `call_service` with permission + cycle checks |
| `packages/wavs/src/subsystems/engine/rpc_caller.rs` | `RpcCallerImpl` with `AllowedCallers` enforcement |
| `examples/components/_helpers/src/` | `export_layer_agent_world!` macro for agent components |
| `examples/components/multi-step-agent/` | 4-step continuation agent example |
| `examples/components/composition-agent/` | Agent that calls utility-service via RPC |
| `examples/components/utility-service/` | Simple callee service accepting RPC calls |
| `app/src/components/service/ComponentEditor.tsx` | Editors for `allowed_service_calls`, `allowed_callers`, `max_continuation_steps` |
| `app/src/components/service/WorkflowViewer.tsx` | Read-only display of the same fields |
| `app/src/pages/services/ServiceDetailPage.tsx` | Surfaces v3.0 fields on existing services |
| `app/src/types/index.ts` | New `AllowedServiceCalls` / `AllowedCallers` types |
| `scripts/test-agent-deploy.sh` | App-driven QA: upload, deploy, fire triggers for all three v3.0 components |
