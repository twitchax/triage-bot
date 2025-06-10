# Development Guide

## Prerequisites

Before developing triage-bot, ensure you have the following dependencies installed:

### Required
- **Rust** (latest stable)
- **Node.js** (version 20 or later) - Required for MCP (Model Context Protocol) server support
- **NPX** - Usually installed with Node.js

### Recommended  
- **sccache** - For faster builds
- **mold linker** - For faster linking (Linux)
- **cargo-nextest** - For running tests

## Quick Setup

Run the automated setup script:

```bash
./utilities/setup_copilot_env.sh
```

This script will install all required dependencies including Node.js.

## Manual Setup

If you prefer manual setup:

1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Install Node.js (Ubuntu): `curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash - && sudo apt-get install -y nodejs`
3. Install build tools: `cargo install sccache cargo-nextest`

## Building and Testing

```bash
# Build the project
cargo build

# Run tests (requires Node.js for MCP tests)
cargo nextest run
```

Note: Tests depend on `npx` to run MCP servers, so Node.js is required even for development.

## TODO

- Abstract each of the services into features, so that we can setup possible separate implementations.
- Add images of the bot working in Slack.
- Threads in slack could be used to keep the conversation going.  So, we could correlate the `thread_ts` to the _first_ LLM request id, and then use that to make subsequent requests.
  Would likely save money on the OpenAI API, and also make it easier to follow conversations.

## Cool Ideas

## Fly Deploy Notes

```bash
fly deploy -c .hidden/fly.toml --local-only
```