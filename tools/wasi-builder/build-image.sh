#!/usr/bin/env bash
set -euo pipefail

# Simple Docker build using BuildKit for cache mounts.
# Usage:
#   tools/wasi-builder/build-image.sh
#
# Env overrides for build args:
#   IMAGE_TAG (default: wavs-wasi-builder:local)
#   RUST_VERSION, TARGET, CARGO_COMPONENT_VERSION, WASM_TOOLS_VERSION, WKG_VERSION

ROOT_DIR=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
IMAGE_TAG="${IMAGE_TAG:-wavs-wasi-builder:local}"

BUILD_ARGS=()
[[ -n "${RUST_VERSION:-}" ]] && BUILD_ARGS+=(--build-arg RUST_VERSION="$RUST_VERSION")
[[ -n "${TARGET:-}" ]] && BUILD_ARGS+=(--build-arg TARGET="$TARGET")
[[ -n "${CARGO_COMPONENT_VERSION:-}" ]] && BUILD_ARGS+=(--build-arg CARGO_COMPONENT_VERSION="$CARGO_COMPONENT_VERSION")
[[ -n "${WASM_TOOLS_VERSION:-}" ]] && BUILD_ARGS+=(--build-arg WASM_TOOLS_VERSION="$WASM_TOOLS_VERSION")
[[ -n "${WKG_VERSION:-}" ]] && BUILD_ARGS+=(--build-arg WKG_VERSION="$WKG_VERSION")

CONTEXT_DIR="$ROOT_DIR/tools/wasi-builder"
DOCKERFILE="$CONTEXT_DIR/Dockerfile"

# Enable BuildKit for cache mounts in Dockerfile
export DOCKER_BUILDKIT=1

echo "[wasi-builder-image] Building $IMAGE_TAG with docker build"
docker build \
  -f "$DOCKERFILE" \
  -t "$IMAGE_TAG" \
  "${BUILD_ARGS[@]}" \
  "$CONTEXT_DIR"

echo "[wasi-builder-image] Built $IMAGE_TAG"
