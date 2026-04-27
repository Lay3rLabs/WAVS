#!/bin/bash
# Test script for deploying v3.0 agent components via the dev API.
#
# Usage:
#   ./scripts/test-agent-deploy.sh [WAVS_URL] [--no-trigger]
#
# Defaults to http://127.0.0.1:8041 (Tauri app's embedded WAVS node).
# Pass http://127.0.0.1:8000 for a standalone WAVS node.
#
# By default the script also fires a manual trigger at each deployed
# service and waits for the result. Pass --no-trigger to deploy only.
#
# Each example service.json ships with manager.address = 0x0...0, so all
# three would collide on a single node (ServiceId = sha256(manager)).
# We patch each one with a unique address in flight (utility=...01,
# multi-step=...02, composition=...03) so they coexist.

set -euo pipefail

WAVS="http://127.0.0.1:8041"
TRIGGER=true
for arg in "$@"; do
  case "$arg" in
    --no-trigger) TRIGGER=false ;;
    http*)        WAVS="$arg" ;;
    *)            echo "Unknown arg: $arg" >&2; exit 2 ;;
  esac
done

COMPONENTS_DIR="examples/build/components"
CONFIGS_DIR="examples/components"

# Unique 20-byte EVM addresses (no 0x, lowercase, 40 hex chars) per service.
UTILITY_ADDR="0000000000000000000000000000000000000001"
MULTISTEP_ADDR="0000000000000000000000000000000000000002"
COMPOSITION_ADDR="0000000000000000000000000000000000000003"
CHAIN_KEY="evm:31337"

green() { printf '\033[32m%s\033[0m\n' "$*"; }
red()   { printf '\033[31m%s\033[0m\n' "$*"; }
info()  { printf '\033[36m→ %s\033[0m\n' "$*"; }

# Decode a hex string ("00ff...") into raw bytes on stdout. Pure printf —
# works without xxd / python.
hex_to_bin() {
  local hex="$1"
  local i
  for ((i=0; i<${#hex}; i+=2)); do
    printf '\x'"${hex:i:2}"
  done
}

# ServiceId = sha256(b"evm" || chain_key_bytes || address_bytes), hex.
# Mirrors `impl From<&ServiceManager> for ServiceId` in
# packages/types/src/service.rs.
compute_service_id() {
  local addr_hex="$1"
  {
    printf 'evm'
    printf '%s' "$CHAIN_KEY"
    hex_to_bin "$addr_hex"
  } | sha256sum | awk '{print $1}'
}

upload_and_deploy() {
  local name="$1"
  local wasm="$2"
  local config="$3"
  local addr_hex="$4"
  # Optional 5th arg: a JSON object merged into workflows.default.component.config.
  # Used to inject runtime config the component reads via host::config_var().
  # Default to {} (no-op merge); bash ${VAR:-default} can't carry a literal
  # `{}` in the default slot because the inner `}` ends the expansion.
  local extra_config="${5-}"
  [ -z "$extra_config" ] && extra_config='{}'

  info "Uploading $name component..."
  local upload_resp
  upload_resp=$(curl -sf -X POST "$WAVS/dev/components" --data-binary @"$wasm")
  local digest
  digest=$(echo "$upload_resp" | jq -r '.digest')

  if [ -z "$digest" ] || [ "$digest" = "null" ]; then
    red "FAIL: Upload $name — no digest returned"
    echo "$upload_resp"
    return 1
  fi
  green "  Uploaded: ${digest:0:16}..."

  info "Deploying $name service..."
  local service_json
  service_json=$(jq \
    --arg d "$digest" \
    --arg a "0x$addr_hex" \
    --argjson c "$extra_config" \
    '
      .workflows.default.component.source.digest = $d
      | .manager.evm.address = $a
      | .workflows.default.component.config += $c
    ' "$config")

  local save_resp
  save_resp=$(echo "$service_json" | curl -sf -X POST "$WAVS/dev/services" \
    -H "Content-Type: application/json" -d @-)
  local hash
  hash=$(echo "$save_resp" | jq -r '.hash')

  if [ -z "$hash" ] || [ "$hash" = "null" ]; then
    red "FAIL: Save $name — no hash returned"
    echo "$save_resp"
    return 1
  fi

  curl -sf -X POST "$WAVS/dev/services/$hash" > /dev/null
  green "  Deployed: $hash (manager.address=0x$addr_hex)"
}

fire_trigger() {
  local name="$1"
  local addr_hex="$2"
  local payload_json="$3"  # raw inner JSON, e.g. {"prompt":"qa"}

  local service_id
  service_id=$(compute_service_id "$addr_hex")

  # SimulatedTriggerRequest. wait_for_completion is FALSE on purpose:
  # the server-side wait polls submission_manager.metrics.get_request_count
  # (packages/wavs/src/http/handlers/debug.rs:69-87), which only advances
  # when the submission subsystem actually submits a result. All three v3.0
  # example services use `submit: "none"`, so the counter never advances
  # and the request hangs forever. We fire-and-forget here; the tester
  # observes results in the app's run history.
  #
  # TriggerData has no rename_all so the variant is "Raw" (PascalCase).
  # Bytes are encoded as a JSON array of u8 ints (od -An -v -tu1).
  local data_bytes
  data_bytes=$(printf '%s' "$payload_json" | od -An -v -tu1 \
    | tr -s ' \n' ',' | sed 's/^,//;s/,$//')
  data_bytes="[${data_bytes}]"

  local body
  body=$(jq -n \
    --arg sid "$service_id" \
    --arg wf  "default" \
    --argjson bytes "$data_bytes" \
    '{
      service_id: $sid,
      workflow_id: $wf,
      trigger: "manual",
      data: { Raw: $bytes },
      count: 1,
      wait_for_completion: false
    }')

  info "Triggering $name (service_id=${service_id:0:16}...)..."
  if ! echo "$body" | curl -sf --max-time 10 -X POST "$WAVS/dev/triggers" \
       -H "Content-Type: application/json" -d @- > /dev/null; then
    red "FAIL: Trigger $name"
    return 1
  fi
  green "  Trigger accepted — check the app's run history for the result."
}

echo "============================================"
echo "  WAVS v3.0 Agent Deploy Test"
echo "  Target:  $WAVS"
echo "  Trigger: $TRIGGER"
echo "============================================"
echo

info "Checking WAVS node..."
if ! curl -sf "$WAVS/health" > /dev/null 2>&1; then
  red "FAIL: Cannot reach $WAVS/health"
  echo "Is the WAVS node / Tauri app running?"
  exit 1
fi
green "Node is up"
echo

# 1. Deploy utility-service (callee must exist before caller fires).
if [ -f "$COMPONENTS_DIR/utility_service.wasm" ]; then
  upload_and_deploy "utility-service" \
    "$COMPONENTS_DIR/utility_service.wasm" \
    "$CONFIGS_DIR/utility-service/service.json" \
    "$UTILITY_ADDR"
  echo
else
  info "Skipping utility-service (wasm not found)"
  echo
fi

# 2. Deploy multi-step agent.
if [ -f "$COMPONENTS_DIR/multi_step_agent.wasm" ]; then
  upload_and_deploy "multi-step-agent" \
    "$COMPONENTS_DIR/multi_step_agent.wasm" \
    "$CONFIGS_DIR/multi-step-agent/service.json" \
    "$MULTISTEP_ADDR"
  echo
else
  info "Skipping multi-step-agent (wasm not found)"
  echo
fi

# 3. Deploy composition agent. It reads `callee_service_id` from
#    component.config (see examples/components/composition-agent/src/lib.rs:49)
#    so it knows which service to RPC into. Inject utility-service's id.
if [ -f "$COMPONENTS_DIR/composition_agent.wasm" ]; then
  utility_sid=$(compute_service_id "$UTILITY_ADDR")
  upload_and_deploy "composition-agent" \
    "$COMPONENTS_DIR/composition_agent.wasm" \
    "$CONFIGS_DIR/composition-agent/service.json" \
    "$COMPOSITION_ADDR" \
    "{\"callee_service_id\": \"$utility_sid\"}"
  echo
else
  info "Skipping composition-agent (wasm not found)"
  echo
fi

if [ "$TRIGGER" = false ]; then
  echo "============================================"
  green "  Deploy complete (--no-trigger). Open the app to inspect."
  echo "============================================"
  exit 0
fi

echo "--- Firing triggers ---"
echo

# Order matters for composition: utility-service is the callee, fire it
# first as a sanity check, then composition-agent so the RPC path runs.
[ -f "$COMPONENTS_DIR/utility_service.wasm" ] && \
  fire_trigger "utility-service"   "$UTILITY_ADDR" \
  '{"op":"echo","data":"qa-test"}' && echo

[ -f "$COMPONENTS_DIR/multi_step_agent.wasm" ] && \
  fire_trigger "multi-step-agent"  "$MULTISTEP_ADDR" \
  '{"prompt":"qa-test"}' && echo

[ -f "$COMPONENTS_DIR/composition_agent.wasm" ] && \
  fire_trigger "composition-agent" "$COMPOSITION_ADDR" \
  '{"prompt":"qa-test","target":"utility-service"}' && echo

echo "============================================"
green "  Done. Open the app to inspect deployed services and runs."
echo "============================================"
