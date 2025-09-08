#!/usr/bin/env bash
set -euo pipefail

# Run the WASI builder image to compile components.
# Usage:
#   tools/wasi-builder/build-components.sh                 # build all
#   tools/wasi-builder/build-components.sh echo-data       # one
#   tools/wasi-builder/build-components.sh a b c           # several
#   tools/wasi-builder/build-components.sh --artifact name # copy only specific artifact name.wasm
# Env:
#   WASI_BUILDER_IMAGE (default: ghcr.io/lay3rlabs/wavs-wasi-builder:latest)
#   PLATFORM (default: host-detected or linux/amd64)

ROOT_DIR=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
ARTIFACT=""
FILTERS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --artifact)
      if [ $# -lt 2 ]; then echo "--artifact requires a value" >&2; exit 2; fi
      ARTIFACT="$2"; shift 2 ;;
    --)
      shift; break ;;
    --*)
      echo "unknown option: $1" >&2; exit 2 ;;
    *)
      FILTERS+=("$1"); shift ;;
  esac
done

if [ ${#FILTERS[@]} -eq 0 ]; then
  COMPONENT_FILTERS=("*")
else
  COMPONENT_FILTERS=("${FILTERS[@]}")
fi

IMAGE_TAG="${WASI_BUILDER_IMAGE:-ghcr.io/lay3rlabs/wavs-wasi-builder:latest}"
# Determine platform if not provided
if [ -z "${PLATFORM:-}" ]; then
  arch=$(uname -m 2>/dev/null || echo "")
  case "$arch" in
    arm64|aarch64) PLATFORM="linux/arm64" ;;
    *) PLATFORM="linux/amd64" ;;
  esac
fi

OUT_DIR="$ROOT_DIR/examples/build/components"
mkdir -p "$OUT_DIR"

# Clean output dir if building all (avoid stale artifacts)
if [ ${#COMPONENT_FILTERS[@]} -eq 1 ] && [ "${COMPONENT_FILTERS[0]}" = "*" ]; then
  rm -f "$OUT_DIR"/*.wasm 2>/dev/null || true
fi

shopt -s nullglob
for filter in "${COMPONENT_FILTERS[@]}"; do
  for cargo_toml in "$ROOT_DIR"/examples/components/$filter/Cargo.toml; do
    comp_dir=$(dirname "$cargo_toml")
    base=$(basename "$comp_dir")
    if [ "$base" = "_helpers" ] || [ "$base" = "_types" ]; then
      continue
    fi

    rel_dir=${comp_dir#"$ROOT_DIR/"}
    echo "[wasi-build] Building component: $rel_dir"

    docker run --rm \
      --platform "$PLATFORM" \
      -v "$ROOT_DIR:/docker" \
      --mount type=volume,source=wavs_wasi_target_cache,target=/target \
      --mount type=volume,source=wavs_wasi_registry_cache,target=/usr/local/cargo/registry \
      --mount type=volume,source=wavs_wasi_git_cache,target=/usr/local/cargo/git \
      -e CARGO_TARGET_DIR=/target \
      -e CARGO_HOME=/usr/local/cargo \
      "$IMAGE_TAG" "$rel_dir" ${ARTIFACT:+--artifact "$ARTIFACT"}
  done
done

# Create checksums file
if command -v sha256sum >/dev/null 2>&1; then
  if compgen -G "$OUT_DIR"/*.wasm >/dev/null; then
    (cd "$ROOT_DIR" && sha256sum -- "./${OUT_DIR#$ROOT_DIR/}"/*.wasm) | tee "$ROOT_DIR/checksums.txt"
  else
    echo "[wasi-build] No wasm artifacts found; skipping checksum generation" >&2
  fi
else
  echo "sha256sum not found; skipping checksum generation" >&2
fi

echo "[wasi-build] Done. Artifacts at: $OUT_DIR"
