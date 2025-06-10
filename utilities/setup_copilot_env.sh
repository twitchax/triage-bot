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

# Install Node.js if not already installed
if ! command -v node &> /dev/null; then
    echo "Installing Node.js..."
    # Install Node.js via NodeSource repository
    curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
    sudo apt-get install -y nodejs
else
    echo "Node.js is already installed ($(node --version))"
fi

# Set environment variables
export RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=mold"

echo "Environment setup complete. You can now build the project with:"
echo "cargo build"
echo "And run tests with:"
echo "cargo nextest run"
echo ""
echo "Note: Tests require Node.js and npx for running MCP servers."