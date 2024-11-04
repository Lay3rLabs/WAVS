FROM rust:1.81-bookworm AS builder
WORKDIR /myapp

# This whole pile will pre-build and cache the dependencies, so we just recompile local code below
COPY Cargo.lock Cargo.toml /myapp/
COPY dummy.rs /myapp/src/main.rs
COPY dummy.rs /myapp/benches/mock_bench.rs
RUN cargo build --release

# clean up these fake local deps so we compile for real later
RUN rm /myapp/src/*.rs
RUN rm -rf target/release/.fingerprint/wasmatic*

# This build step should just compile the local code and be faster
COPY . .
RUN cargo build --release

### PRODUCTION

# Now, pack up that binary in a nice small image
FROM debian:bookworm-slim
WORKDIR /wasmatic

RUN apt-get update && apt-get upgrade -y
RUN apt install -y libcurl4
COPY --from=builder /myapp/target/release/wasmatic /usr/local/bin/wasmatic
COPY --from=builder /myapp/wasmatic.toml /etc/wasmatic/wasmatic.toml
EXPOSE 8081
ENTRYPOINT [ "/usr/local/bin/wasmatic" ]
CMD ["up"]
