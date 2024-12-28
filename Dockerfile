FROM rust:1.81-bookworm AS builder
WORKDIR /myapp

# This whole pile will pre-build and cache the dependencies, so we just recompile local code below
COPY Cargo.lock Cargo.toml /myapp/
COPY packages/aggregator /myapp/packages/aggregator
COPY packages/wavs/Cargo.toml /myapp/packages/wavs/Cargo.toml
COPY packages/utils/Cargo.toml /myapp/packages/utils/Cargo.toml
COPY dummy.rs /myapp/packages/wavs/benches/mock_bench.rs
COPY dummy.rs /myapp/packages/wavs/src/mock_bench.rs
COPY dummy.rs /myapp/packages/utils/src/lib.rs
RUN cargo build --manifest-path /myapp/packages/wavs/Cargo.toml --release

# clean up these fake local deps so we compile for real later
RUN rm /myapp/packages/wavs/src/*.rs
RUN rm /myapp/packages/wavs/benches/*.rs
RUN rm /myapp/packages/utils/src/*.rs
RUN rm -rf target/release/.fingerprint/wavs*

# This build step should just compile the local code and be faster
COPY . .
RUN cargo build --manifest-path /myapp/packages/wavs/Cargo.toml --release

### PRODUCTION

# Now, pack up that binary in a nice small image
FROM debian:bookworm-slim
WORKDIR /wavs

RUN apt-get update && apt-get upgrade -y
RUN apt install -y libcurl4
COPY --from=builder /myapp/target/release/wavs /usr/local/bin/wavs
COPY --from=builder /myapp/packages/wavs/wavs.toml /etc/wavs/wavs.toml
EXPOSE 8000
CMD ["/usr/local/bin/wavs"]
