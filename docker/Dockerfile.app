FROM rust:slim-bookworm AS linker
RUN apt-get update && apt-get install -y mold clang lld && rm -rf /var/lib/apt/lists/*
ENV RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=mold"

FROM linker AS chef
WORKDIR /app
RUN rustup install nightly-2025-05-14
RUN cargo install cargo-chef

FROM chef AS planner
COPY Cargo.toml .
COPY Cargo.lock .
COPY rust-toolchain.toml .
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json --bin triage-bot

FROM chef AS builder
COPY rust-toolchain.toml .
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin triage-bot
COPY Cargo.toml .
COPY Cargo.lock .
COPY rust-toolchain.toml .
COPY src ./src
RUN cargo build --release --bin triage-bot

FROM ubuntu:noble AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates curl
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
RUN apt-get install -y nodejs
RUN apt-get clean && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/triage-bot /usr/local/bin

ENTRYPOINT ["/usr/local/bin/triage-bot"]
