# GitHub Copilot Setup

This repository is configured to work seamlessly with GitHub Copilot. This document provides guidance on how to use the provided tools and configurations.

## Automated Environment Setup

We've included a script to help you set up your development environment. Run:

```bash
../scripts/setup_copilot_env.sh
```

This script will:
1. Install or update Rust and the required toolchain
2. Install sccache for faster builds
3. Install mold linker (on Linux) for faster linking
4. Set up environment variables for optimal builds

## VSCode Integration

If you're using VSCode, we've included recommended extensions and settings:

- Open this repository in VSCode
- You should get a prompt to install the recommended extensions
- The settings are configured for optimal Rust development

## Building the Project

Build the project with:

```bash
cargo build
```

## Running Tests

Run tests with:

```bash
cargo nextest run
```

## GitHub Actions Workflow

A GitHub Actions workflow is configured for GitHub Copilot, which includes:
- Setting up the Rust toolchain
- Installing necessary tools
- Configuring the firewall to allow access to:
  - index.crates.io
  - static.rust-lang.org
  - docs.rs

## Build Conventions

As mentioned in the AGENTS.md file, the project follows these build conventions:

- CI runs on GitHub Actions. Keep the badge green.
- Use a local `sccache` for repeat builds.