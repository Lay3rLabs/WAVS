FROM rust:1.81-bookworm AS builder
WORKDIR /myapp

RUN apt update

# This whole pile will pre-build and cache the dependencies, so we just recompile local code below
COPY Cargo.lock Cargo.toml /myapp/
COPY dummy.rs /myapp/src/main.rs

# copy over the examples as placeholders
COPY examples/btc-avg/Cargo.toml /myapp/examples/btc-avg/Cargo.toml
COPY dummy.rs /myapp/examples/btc-avg/src/lib.rs
COPY examples/square/Cargo.toml /myapp/examples/square/Cargo.toml
COPY dummy.rs /myapp/examples/square/src/lib.rs

RUN cargo build --release

# clean up these fake local deps so we compile for real later
RUN rm /myapp/src/*.rs
RUN rm -rf target/release/.fingerprint/{wasmatic,square,btc-avg}*

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
EXPOSE 8081
ENTRYPOINT [ "/usr/local/bin/wasmatic" ]
CMD ["up"]
