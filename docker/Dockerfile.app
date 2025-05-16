FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin triage-bot

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates
RUN apt-get install -y libssl-dev
RUN apt-get clean && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/triage-bot /usr/local/bin
ENTRYPOINT ["/usr/local/bin/triage-bot"]
