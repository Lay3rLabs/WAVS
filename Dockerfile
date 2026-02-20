# Base image with common dependencies
FROM rust:1.90-slim-bookworm AS base
RUN apt-get update && apt-get install -y pkg-config libssl-dev ca-certificates curl && rm -rf /var/lib/apt/lists/*

# This whole pile will pre-build and cache the dependencies, so we just recompile local code below
FROM base AS planner
WORKDIR /myapp
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM base AS cacher
WORKDIR /myapp
RUN cargo install cargo-chef
COPY --from=planner /myapp/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# This build step should just compile the local code and be faster
FROM base AS builder
WORKDIR /myapp
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /myapp/target target
RUN cargo build --release

### PRODUCTION

# Pinned foundry version
FROM --platform=linux/amd64 ghcr.io/foundry-rs/foundry:v0.3.0 AS foundry

# Now, pack up that binary in a nice small image
FROM debian:bookworm-slim
WORKDIR /wavs

RUN apt-get update && apt-get upgrade -y
RUN apt install -y libcurl4 jq

COPY --from=builder /myapp/target/release/wavs /usr/local/bin/wavs
COPY --from=builder /myapp/wavs.toml /var/wavs/wavs.toml

COPY --from=builder /myapp/target/release/wavs-cli /usr/local/bin/wavs-cli

# copy /usr/local/bin/forge, cast, anvil, and chisel from foundry
COPY --from=foundry /usr/local/bin/forge /usr/local/bin/forge
COPY --from=foundry /usr/local/bin/cast /usr/local/bin/cast
COPY --from=foundry /usr/local/bin/anvil /usr/local/bin/anvil
COPY --from=foundry /usr/local/bin/chisel /usr/local/bin/chisel

EXPOSE 8000 8001
CMD ["/usr/local/bin/wavs"]
