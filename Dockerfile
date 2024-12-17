FROM rust:slim

LABEL maintainer="tom@cyborgnetwork.io"
LABEL description="Demo container for the Cyborg Oracle Feeder"

WORKDIR /root
COPY . /root

RUN apt-get update && apt-get install -y \
    sudo \
    curl \
    git \
    wget \
    bash \
    build-essential \
    gcc \
    clang \
    pkg-config \
    libssl-dev \
    libffi-dev \
    libzmq3-dev \
    && apt-get clean


RUN cargo build --release

# For testnet: wss://fraa-dancebox-3131-rpc.a.dancebox.tanssi.network
# For local docker: ws://127.0.0.1:9944
ENV PARACHAIN_URL=ws://127.0.0.1:9988
ENV ACCOUNT_SEED="bottom drive obey lake curtain smoke basket hold race lonely fit walk//Alice"

CMD /bin/bash -c "./target/release/cyborg-oracle-feeder start --parachain-url $PARACHAIN_URL --account-seed '$ACCOUNT_SEED'"
