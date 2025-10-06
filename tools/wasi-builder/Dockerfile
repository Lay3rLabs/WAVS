# syntax=docker/dockerfile:1.7-labs

ARG RUST_VERSION=1.90-slim-bookworm
ARG CARGO_COMPONENT_VERSION=0.21.1
ARG WASM_TOOLS_VERSION=1.238.1
ARG WKG_VERSION=0.12.0
ARG TARGET=wasm32-wasip1
ARG BUILDPLATFORM=linux/amd64

FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}

ARG TARGET
ARG CARGO_COMPONENT_VERSION
ARG WASM_TOOLS_VERSION
ARG WKG_VERSION

ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:/usr/local/rustup/bin:$PATH \
    CARGO_TERM_COLOR=never

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates \
      git \
      pkg-config \
      build-essential \
      libssl-dev \
 && rm -rf /var/lib/apt/lists/*

# Install toolchain and required tooling with BuildKit cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    rustup target add ${TARGET} \
    && cargo install --locked cargo-component@${CARGO_COMPONENT_VERSION} \
    && cargo install --locked wasm-tools@${WASM_TOOLS_VERSION} \
    && cargo install --locked wkg@${WKG_VERSION}

WORKDIR /work
COPY entrypoint.sh /usr/local/bin/entrypoint
RUN chmod +x /usr/local/bin/entrypoint
ENTRYPOINT ["/usr/local/bin/entrypoint"]
