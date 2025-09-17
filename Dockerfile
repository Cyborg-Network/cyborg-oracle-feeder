FROM rustlang/rust:nightly-slim as builder

LABEL maintainer="tom@cyborgnetwork.io"
LABEL description="Demo container for the Cyborg Oracle Feeder"

WORKDIR /app
COPY . /app

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

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    bash \
    libssl3 \
    libffi-dev \
    libzmq3-dev \
    && apt-get clean


WORKDIR /app

COPY --from=builder /app/target/release/cyborg-oracle-feeder .

ENV ACCOUNT_SEED="//Eve"

CMD /bin/bash -c "./cyborg-oracle-feeder start --parachain-url $PARACHAIN_URL --account-seed '$ACCOUNT_SEED'"
