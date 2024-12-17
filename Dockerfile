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

ENV PARACHAIN_URL=
ENV ACCOUNT_SEED=

CMD /bin/bash -c "./target/release/cyborg-oracle-feeder start --parachain-url $PARACHAIN_URL --account-seed '$ACCOUNT_SEED'"
