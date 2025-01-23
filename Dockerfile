# This whole pile will pre-build and cache the dependencies, so we just recompile local code below
FROM rust:1.84-bookworm AS planner
WORKDIR /myapp
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM rust:1.84-bookworm AS cacher
WORKDIR /myapp
RUN cargo install cargo-chef
COPY --from=planner /myapp/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# This build step should just compile the local code and be faster
FROM rust:1.84-bookworm AS builder
WORKDIR /myapp
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /myapp/target target
RUN cargo build --release

### PRODUCTION

# Now, pack up that binary in a nice small image
FROM debian:bookworm-slim
WORKDIR /wavs

RUN apt-get update && apt-get upgrade -y
RUN apt install -y libcurl4 jq

COPY --from=builder /myapp/target/release/wavs /usr/local/bin/wavs
COPY --from=builder /myapp/packages/wavs/wavs.toml /var/wavs/wavs.toml

COPY --from=builder /myapp/target/release/wavs-cli /usr/local/bin/wavs-cli
COPY --from=builder /myapp/packages/cli/wavs-cli.toml /var/wavs-cli/wavs-cli.toml

COPY --from=builder /myapp/target/release/wavs-aggregator /usr/local/bin/wavs-aggregator
COPY --from=builder /myapp/packages/aggregator/wavs-aggregator.toml /var/wavs-aggregator/wavs-aggregator.toml

EXPOSE 8000 8001
CMD ["/usr/local/bin/wavs"]
