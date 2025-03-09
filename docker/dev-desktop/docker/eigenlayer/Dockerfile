FROM rust
RUN apt update && apt install -y jq && apt install -y nodejs npm && npm install -g yarn
RUN curl -L https://foundry.paradigm.xyz | bash
RUN ~/.foundry/bin/foundryup 
ENV PATH="/root/.foundry/bin:${PATH}"
RUN git clone https://github.com/BreadchainCoop/avs-ecdsa-sol-sdk.git
RUN cd avs-ecdsa-sol-sdk && yarn install && yarn build
RUN git clone --recurse-submodules https://github.com/BreadchainCoop/wavs-middleware.git
RUN cd wavs-middleware && cargo build --release
WORKDIR /wavs-middleware
RUN cargo build 
COPY start.sh /wavs-middleware/start.sh
RUN chmod +x /wavs-middleware/start.sh
CMD ["/bin/sh", "-c", "./start.sh"]
