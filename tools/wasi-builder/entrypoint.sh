#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "usage: entrypoint <relative-path-to-component-crate> [--debug] [--artifact NAME_OR_FILE]" >&2
  exit 1
fi

COMPONENT_DIR="$1"
MODE="release"
ARTIFACT=""
shift || true

while [ $# -gt 0 ]; do
  case "${1:-}" in
    --debug)
      MODE="debug"; shift ;;
    --artifact)
      if [ $# -lt 2 ]; then
        echo "--artifact requires a value" >&2; exit 2
      fi
      ARTIFACT="$2"; shift 2 ;;
    *)
      echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

DOCKER_DIR="/docker"
OUT_DIR="$DOCKER_DIR/examples/build/components"
TARGET_DIR="${CARGO_TARGET_DIR:-$DOCKER_DIR/target}"

mkdir -p "$OUT_DIR"

cd "$DOCKER_DIR/$COMPONENT_DIR"

# Ensure reproducible builds
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-0}
export CARGO_BUILD_INCREMENTAL=false
export CARGO_PROFILE_RELEASE_LTO="thin"
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
export CARGO_PROFILE_RELEASE_PANIC="abort"
umask 022

if [ "$MODE" = "release" ]; then
  cargo component build --release --locked
else
  cargo component build --locked
fi

# Copy resulting wasm(s) to the repo output folder
ART_DIR="$TARGET_DIR/wasm32-wasip1/$MODE"

if [ -n "$ARTIFACT" ]; then
  name=$(basename -- "$ARTIFACT")
  case "$name" in
    *.wasm) : ;;
    *) name="$name.wasm" ;;
  esac
  if [ ! -f "$ART_DIR/$name" ]; then
    echo "artifact not found: $ART_DIR/$name" >&2
    exit 3
  fi
  cp -f "$ART_DIR/$name" "$OUT_DIR/"
  chmod 0644 "$OUT_DIR/$name" || true
else
  find "$ART_DIR" -maxdepth 1 -type f -name '*.wasm' -print0 \
    | while IFS= read -r -d '' f; do
        cp -f "$f" "$OUT_DIR/"
      done
  chmod 0644 "$OUT_DIR"/*.wasm 2>/dev/null || true
fi

echo "Artifacts copied to: $OUT_DIR"
