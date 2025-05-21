#!/bin/bash
# Copilot startup script - helps set up the development environment for triage-bot

set -e

echo "Setting up development environment for triage-bot..."

# Install or update Rust if needed
if ! command -v rustup &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "Updating Rust..."
    rustup update
fi

# Install the specific toolchain from rust-toolchain.toml
echo "Installing specified Rust toolchain..."
rustup show

# Install cargo-nextest if not already installed
if ! command -v cargo-nextest &> /dev/null; then
    echo "Installing cargo-nextest..."
    cargo install cargo-nextest
fi

# Install sccache if not already installed
if ! command -v sccache &> /dev/null; then
    echo "Installing sccache..."
    cargo install sccache
fi

# Install mold linker if not already installed (Linux only)
if [[ "$(uname)" == "Linux" ]] && ! command -v mold &> /dev/null; then
    echo "Installing mold linker..."
    sudo apt-get update
    sudo apt-get install -y clang mold
fi

# Set environment variables
export CARGO_PROFILE_RELEASE_LTO=true
export RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=mold"

echo "Environment setup complete. You can now build the project with:"
echo "CARGO_PROFILE_RELEASE_LTO=true RUSTFLAGS=\"-C linker=clang -C link-arg=-fuse-ld=mold\" cargo build --release -j 64"
echo "And run tests with:"
echo "cargo nextest run"