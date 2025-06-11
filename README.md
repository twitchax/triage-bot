[![Build and Test](https://github.com/twitchax/triage-bot/actions/workflows/build.yml/badge.svg)](https://github.com/twitchax/triage-bot/actions/workflows/build.yml)
[![codecov](https://codecov.io/gh/twitchax/triage-bot/branch/main/graph/badge.svg?token=35MZN0YFZF)](https://codecov.io/gh/twitchax/triage-bot)
[![Version](https://img.shields.io/crates/v/triage-bot.svg)](https://crates.io/crates/triage-bot)
[![Crates.io](https://img.shields.io/crates/d/triage-bot?label=crate)](https://crates.io/crates/triage-bot)
[![GitHub all releases](https://img.shields.io/github/downloads/twitchax/triage-bot/total?label=binary)](https://github.com/twitchax/triage-bot/releases)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# triage-bot

An intelligent AI-powered triage bot that transforms your Slack support channels into efficient, automated help desks. Built with OpenAI integration, triage-bot automatically:

- 🏷️ **Tags the right people** - Identifies and notifies relevant on-call personnel
- 📊 **Classifies issues** - Categorizes problems as bugs, features, questions, or incidents  
- 🔍 **Provides context** - Searches past discussions and suggests solutions
- 🧠 **Learns continuously** - Remembers team knowledge and improves over time
- 🔧 **Extends capabilities** - Integrates with external tools via Model Context Protocol (MCP)

Perfect for engineering teams, customer support, and any organization looking to streamline their Slack-based support workflow.

## Quick Start

1. **Install triage-bot** (see [Installation](#installation) for platform-specific instructions)
2. **Set up prerequisites** (Node.js 20+ for MCP support, if you use `npx`)
3. **Configure your environment** (OpenAI API key, Slack tokens, database)
4. **Run the bot** and add it to your Slack channels

See the [Configuration](#configuration) section for detailed setup instructions.

## Installation

### Binary Installation

**Windows:**

```powershell
$ iwr https://github.com/twitchax/triage-bot/releases/latest/download/triage-bot_x86_64-pc-windows-gnu.zip
$ Expand-Archive triage-bot_x86_64-pc-windows-gnu.zip -DestinationPath C:\Users\%USERNAME%\AppData\Local\Programs\triage-bot
```

**macOS (Apple Silicon):**

```bash
$ curl -LO https://github.com/twitchax/triage-bot/releases/latest/download/triage-bot_aarch64-apple-darwin.zip
$ unzip triage-bot_aarch64-apple-darwin.zip -d /usr/local/bin
$ chmod a+x /usr/local/bin/triage-bot
```

**Linux:**

```bash
$ curl -LO https://github.com/twitchax/triage-bot/releases/latest/download/triage-bot_x86_64-unknown-linux-gnu.zip
$ unzip triage-bot_x86_64-unknown-linux-gnu.zip -d /usr/local/bin
$ chmod a+x /usr/local/bin/triage-bot
```

### From Source

**Using Cargo:**

```bash
$ cargo install triage-bot
```

## Prerequisites

**Node.js (Required for mainstream MCP Support)**

Triage-bot requires Node.js (version 20 or later) to run Model Context Protocol (MCP) servers that use `npx` (fairly standard at the moment), which provide additional tools and capabilities to extend the bot's functionality.  If you use remote MCPs, or don't use `npx`, you can skip this requirement.

**Installation by Platform:**
- **Ubuntu/Debian**: `curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash - && sudo apt-get install -y nodejs`
- **macOS**: `brew install node` (requires [Homebrew](https://brew.sh/))
- **Windows**: Download from [nodejs.org](https://nodejs.org/)

**Other Requirements:**
- OpenAI API account with API key
- Slack workspace with bot permissions
- SurrealDB instance (for storing configurations and message history)

## How It Works

Triage-bot seamlessly integrates with your Slack workspace to provide intelligent support automation with minimal setup required.

### Basic Workflow

1. **Message Detection** - Bot monitors configured Slack channels for new messages
2. **AI Analysis** - Uses OpenAI models to understand the message context and urgency
3. **Intelligent Response** - Takes appropriate actions based on the analysis:
   - 🏷️ Tags relevant on-call personnel
   - 📂 Classifies issues (Bug, Feature, Question, Incident, Other)
   - 🔍 Searches past discussions for relevant context
   - 💡 Suggests potential solutions when confident
   - 📝 Maintains conversation history for future reference

### Examples in Action

Here are real examples of how triage-bot operates in Slack channels:

#### 💬 Basic Question Handling
![Basic Question](assets/basic_question.png)

The bot responds to straightforward questions with helpful information and context, automatically analyzing the message to provide relevant assistance.

#### 🎯 Prime Directive and Channel Guidelines
![Prime Directive](assets/prime_directive.png)

Triage-bot follows a configurable "prime directive" that guides its behavior and priorities within each channel, ensuring consistent and appropriate responses.

#### 🧠 Adding Context and Memory
![Add Context](assets/add_context.png)

Users can teach the bot about their environment by adding context. The bot remembers this information and uses it to provide more accurate assistance in future interactions.

#### 🔧 Advanced Tool Support with MCP
![MCP Support](assets/mcp_support.png)

The bot integrates with Model Context Protocol (MCP) servers to access additional tools and capabilities, extending its functionality beyond basic chat responses (e.g., deepwiki integration shown here).

#### 🔍 Detailed Execution Tracing
![Typical Trace](assets/typical_trace.png)

For transparency and debugging, triage-bot provides detailed execution traces showing how it processes messages and makes decisions. A typical trace shows the "back and forth" between the bot and the LLM, revealing how it arrives at responses.

### User Commands

Interact with triage-bot using these commands:

**Direct Mentions:**
- `@triage-bot why is my build failing?` - Ask for help with specific issues
- `@triage-bot please remember that FooService owns bar-api` - Add context and knowledge
- `@triage-bot reset the channel directive to prioritize security incidents` - Update channel behavior

**💡 Pro Tip:** The bot also responds to top-level comments that don't mention it directly, making conversations feel more natural.

## Configuration

Triage-bot can be configured using environment variables or a configuration file. Environment variables take precedence over config file values.

### Required Configuration

These settings are required for basic operation:

| Environment Variable              | Description                       | Example                 |
| --------------------------------- | --------------------------------- | ----------------------- |
| `TRIAGE_BOT_OPENAI_API_KEY`       | Your OpenAI API key               | `sk-...`                |
| `TRIAGE_BOT_SLACK_APP_TOKEN`      | Slack app token (for socket mode) | `xapp-...`              |
| `TRIAGE_BOT_SLACK_BOT_TOKEN`      | Slack bot user OAuth token        | `xoxb-...`              |
| `TRIAGE_BOT_SLACK_SIGNING_SECRET` | Slack app signing secret          | `abc123...`             |
| `TRIAGE_BOT_DB_ENDPOINT`          | SurrealDB connection URL          | `http://localhost:8000` |
| `TRIAGE_BOT_DB_USERNAME`          | SurrealDB username                | `root`                  |
| `TRIAGE_BOT_DB_PASSWORD`          | SurrealDB password                | `root`                  |

### Model Configuration

Fine-tune AI behavior with these optional settings:

| Environment Variable                                 | Description                                     | Default   |
| ---------------------------------------------------- | ----------------------------------------------- | --------- |
| `TRIAGE_BOT_OPENAI_SEARCH_AGENT_MODEL`               | OpenAI model for search operations              | `gpt-4.1` |
| `TRIAGE_BOT_OPENAI_ASSISTANT_AGENT_MODEL`            | OpenAI model for assistant responses            | `o3`      |
| `TRIAGE_BOT_OPENAI_SEARCH_AGENT_TEMPERATURE`         | Creativity level for search agent (0.0-2.0)     | `0.0`     |
| `TRIAGE_BOT_OPENAI_ASSISTANT_AGENT_TEMPERATURE`      | Creativity level for assistant agent (0.0-2.0)  | `0.7`     |
| `TRIAGE_BOT_OPENAI_SEARCH_AGENT_REASONING_EFFORT`    | Reasoning depth for search (low/medium/high)    | `medium`  |
| `TRIAGE_BOT_OPENAI_ASSISTANT_AGENT_REASONING_EFFORT` | Reasoning depth for assistant (low/medium/high) | `medium`  |
| `TRIAGE_BOT_OPENAI_MAX_TOKENS`                       | Maximum response length                         | `16384`   |

### Custom Directives

Customize bot behavior with these advanced options:

| Environment Variable                        | Description                              | Default  |
| ------------------------------------------- | ---------------------------------------- | -------- |
| `TRIAGE_BOT_SYSTEM_DIRECTIVE`               | Custom system prompt for assistant agent | Built-in |
| `TRIAGE_BOT_MENTION_ADDENDUM_DIRECTIVE`     | Additional instructions for @-mentions   | Built-in |
| `TRIAGE_BOT_SEARCH_AGENT_DIRECTIVE`         | Custom search agent behavior             | Built-in |
| `TRIAGE_BOT_MESSAGE_SEARCH_AGENT_DIRECTIVE` | Custom message search behavior           | Built-in |

### Understanding Reasoning Models

**📖 What are Reasoning Models?**
Reasoning effort parameters only apply to OpenAI's o-series models (like `o1`, `o3`) which are specifically designed for complex reasoning tasks.

**⚙️ Reasoning Effort Levels:**
- `low` - Faster responses with basic reasoning
- `medium` - Balanced performance and reasoning depth (recommended)
- `high` - Deep reasoning at the cost of response time

**🔧 Model Behavior:**
- **O-series models**: Use reasoning effort parameters for computational depth
- **GPT-series models**: Use temperature parameters for response creativity

### Configuration File Option

You can also use a `.hidden/config.toml` file instead of environment variables:

```toml
# Required settings
openai_api_key = "sk-your-api-key-here"
slack_app_token = "xapp-your-app-token"
slack_bot_token = "xoxb-your-bot-token"
slack_signing_secret = "your-signing-secret"
db_endpoint = "http://localhost:8000"
db_username = "root"
db_password = "root"

# Optional: Model configuration
openai_search_agent_model = "gpt-4o"
openai_assistant_agent_model = "o3"

# Optional: Reasoning effort for o-series models
openai_search_agent_reasoning_effort = "medium"
openai_assistant_agent_reasoning_effort = "high"
```

**💡 Priority:** Environment variables override config file values.

### Observability (Optional)

Enable monitoring and tracing with OpenTelemetry:

| Environment Variable          | Description                          | Example                    |
| ----------------------------- | ------------------------------------ | -------------------------- |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP endpoint URL for telemetry data | `http://localhost:4317`    |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | Export protocol (grpc/http)          | `grpc`                     |
| `OTEL_EXPORTER_OTLP_HEADERS`  | Headers for OTLP requests            | `authorization=Bearer ...` |

These follow the [OpenTelemetry specification](https://opentelemetry.io/docs/specs/otel/configuration/sdk-environment-variables/) and work with platforms like Jaeger, Zipkin, or cloud tracing services.

## Getting Started

1. **Install Node.js** (version 20+) - Required for `npx` MCP server support
2. **Install triage-bot** using your preferred method ([see Installation](#installation))
3. **Set up your services:**
   - Create an OpenAI API account and get your API key
   - Set up a Slack app with bot permissions in your workspace
   - Deploy a SurrealDB instance (local or cloud)
4. **Configure environment variables** ([see Configuration](#configuration))
5. **Run the bot:** `triage-bot`
6. **Add the bot to your Slack channels** and start asking questions!

## Troubleshooting

### Common Issues

**"Node.js not found" error:**
- Ensure Node.js 20+ is installed and `node` is in your PATH
- Verify with: `node --version`

**"Database connection failed":**
- Check that SurrealDB is running and accessible
- Verify `TRIAGE_BOT_DB_ENDPOINT` points to the correct URL
- Ensure database credentials are correct

**"OpenAI API errors":**
- Verify your API key is valid and has sufficient credits
- Check that the specified models (e.g., `o3`, `gpt-4o`) are available to your account

**"Slack connection issues":**
- Ensure your Slack app has the correct permissions (chat:write, channels:read, etc.)
- Verify socket mode is enabled in your Slack app settings
- Check that tokens are correctly formatted (app token starts with `xapp-`, bot token with `xoxb-`)

### Debug Mode

Enable verbose logging for troubleshooting:
```bash
RUST_LOG=debug triage-bot
```

## Architecture and Extensibility

Triage-bot is built with a modular, trait-based architecture that makes it easy to extend or replace components.

### Core Components

**🔌 Default Implementations:**
- **Slack Integration** - Socket mode connection for real-time messaging
- **SurrealDB Storage** - Stores channel configs, context, and message history  
- **OpenAI Integration** - Powers AI responses and searches using latest models
- **MCP Support** - Extends capabilities through external tool servers

### Extensibility

**🔧 Plugin Architecture:**
The application uses Rust traits for clean interfaces:

- `GenericChatClient` - Chat platform integration (Slack, Discord, Teams, etc.)
- `GenericDbClient` - Database operations (SurrealDB, PostgreSQL, MongoDB, etc.) 
- `GenericLlmClient` - LLM providers (OpenAI, Anthropic, local models, etc.)

**🛠️ Adding New Integrations:**
To add support for new services, implement the relevant trait:

```rust
use triage_bot::base::types::GenericChatClient;

struct MyCustomChatClient {
    // your implementation
}

impl GenericChatClient for MyCustomChatClient {
    // implement required methods
}
```

This modular design ensures triage-bot can adapt to your existing infrastructure and tooling.

## Development

### Running Tests

```bash
$ cargo nextest run
```

### Contributing

We welcome contributions! Please see the development guide in [DEVELOPMENT.md](DEVELOPMENT.md) for setup instructions and coding conventions.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.