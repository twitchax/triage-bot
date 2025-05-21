# GitHub Copilot Setup

This repository is configured to work seamlessly with GitHub Copilot. This document provides guidance on how to use the provided tools and configurations.

## Automated Environment Setup

We've included a script to help you set up your development environment. Run:

```bash
./scripts/setup_copilot_env.sh
```

This script will:
1. Install or update Rust and the required toolchain
2. Install cargo-nextest for running tests
3. Install sccache for faster builds
4. Install mold linker (on Linux) for faster linking
5. Set up environment variables for optimal builds

## VSCode Integration

If you're using VSCode, we've included recommended extensions and settings:

- Open this repository in VSCode
- You should get a prompt to install the recommended extensions
- The settings are configured for optimal Rust development

## Building the Project

For the fastest, deterministic release build, use:

```bash
CARGO_PROFILE_RELEASE_LTO=true \
RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=mold" \
cargo build --release -j 64
```

## Running Tests

Run tests with cargo-nextest for faster, more reliable test runs:

```bash
cargo nextest run
```

## GitHub Actions Workflow

A GitHub Actions workflow is configured for GitHub Copilot, which includes:
- Setting up the Rust toolchain
- Installing necessary tools
- Configuring the firewall to allow access to:
  - cdn.fwupd.org
  - index.crates.io
  - static.rust-lang.org
  - docs.rs

## Build Conventions

As mentioned in the AGENTS.md file, the project follows these build conventions:

```
# Fast, deterministic release build
CARGO_PROFILE_RELEASE_LTO=true \
RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=mold" \
cargo build --release -j 64
```

- CI runs on GitHub Actions. Keep the badge green.
- Prefer `cargo nextest run` for faster, flaky‑test‑resilient suites.
- Use a local `sccache` for repeat builds.