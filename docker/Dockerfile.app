FROM rust:slim-bookworm AS chef
WORKDIR /app
RUN rustup install nightly-2025-05-14
RUN cargo install cargo-chef

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json --bin triage-bot

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin triage-bot
COPY . .
RUN cargo build --release --bin triage-bot

# We do not need the Rust toolchain to run the binary!
FROM ubuntu:noble AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates
RUN apt-get install -y libssl-dev
RUN apt-get clean && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/triage-bot /usr/local/bin

ENTRYPOINT ["/usr/local/bin/triage-bot"]
