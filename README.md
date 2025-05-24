[![Build and Test](https://github.com/twitchax/triage-bot/actions/workflows/build.yml/badge.svg)](https://github.com/twitchax/triage-bot/actions/workflows/build.yml)
[![codecov](https://codecov.io/gh/twitchax/triage-bot/branch/main/graph/badge.svg?token=35MZN0YFZF)](https://codecov.io/gh/twitchax/triage-bot)
[![Version](https://img.shields.io/crates/v/triage-bot.svg)](https://crates.io/crates/triage-bot)
[![Crates.io](https://img.shields.io/crates/d/triage-bot?label=crate)](https://crates.io/crates/triage-bot)
[![GitHub all releases](https://img.shields.io/github/downloads/twitchax/triage-bot/total?label=binary)](https://github.com/twitchax/triage-bot/releases)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# triage-bot

An OpenAI-powered triage bot for a slack support channel designed to tag oncalls, prioritize issues, suggest solutions, and streamline communication.

## Install

Windows:

```powershell
$ iwr https://github.com/twitchax/triage-bot/releases/latest/download/triage-bot_x86_64-pc-windows-gnu.zip
$ Expand-Archive triage-bot_x86_64-pc-windows-gnu.zip -DestinationPath C:\Users\%USERNAME%\AppData\Local\Programs\triage-bot
```

Mac OS (Apple Silicon):

```bash
$ curl -LO https://github.com/twitchax/triage-bot/releases/latest/download/triage-bot_aarch64-apple-darwin.zip
$ unzip triage-bot_aarch64-apple-darwin.zip -d /usr/local/bin
$ chmod a+x /usr/local/bin/triage-bot
```

Linux:

```bash
$ curl -LO https://github.com/twitchax/triage-bot/releases/latest/download/triage-bot_x86_64-unknown-linux-gnu.zip
$ unzip triage-bot_x86_64-unknown-linux-gnu.zip -d /usr/local/bin
$ chmod a+x /usr/local/bin/triage-bot
```

Cargo:

```bash
$ cargo install triage-bot
```

## Usage

Triage-bot monitors your Slack support channels and automatically assists with user support requests. It integrates with your existing Slack workspace and requires minimal setup to get started.

### Basic Workflow

1. **User posts a message** in a Slack channel where triage-bot is active.
2. **Triage-bot analyzes the message** using LLMs to determine the nature and urgency.
3. **Bot takes appropriate actions**:
   - Tags relevant on-call personnel.
   - Classifies the issue (Bug, Feature, Question, Incident, Other).
   - Provides helpful context from past discussions or web searches.
   - Suggests potential solutions when confident.

### Supported Commands

Users can interact directly with triage-bot by @-mentioning it:

- **Help requests**: `@triage-bot why is my build failing?`
- **Context updates**: `@triage-bot please remember that FooService owns bar-api`
- **Update channel directive**: `@triage-bot reset the channel directive to prioritize security incidents`

Note that top-level comments that don't tag the bot will also be responded to.

### Configuration

Configuration is handled through environment variables or a config file (`.hidden/config.toml`). The bot supports the following configuration options:

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `TRIAGE_BOT_OPENAI_API_KEY` | OpenAI API key | (required) |
| `TRIAGE_BOT_SLACK_APP_TOKEN` | Slack app token | (required) |
| `TRIAGE_BOT_SLACK_BOT_TOKEN` | Slack bot token | (required) |
| `TRIAGE_BOT_SLACK_SIGNING_SECRET` | Slack signing secret | (required) |
| `TRIAGE_BOT_DB_ENDPOINT` | SurrealDB endpoint URL | (required) |
| `TRIAGE_BOT_DB_USERNAME` | SurrealDB username | (required) |
| `TRIAGE_BOT_DB_PASSWORD` | SurrealDB password | (required) |
| `TRIAGE_BOT_OPENAI_SEARCH_AGENT_MODEL` | OpenAI model for search agent | `gpt-4.1` |
| `TRIAGE_BOT_OPENAI_ASSISTANT_AGENT_MODEL` | OpenAI model for assistant agent | `o3-mini` |
| `TRIAGE_BOT_OPENAI_SEARCH_AGENT_TEMPERATURE` | Sampling temperature for search agent | `0.0` |
| `TRIAGE_BOT_OPENAI_ASSISTANT_AGENT_TEMPERATURE` | Sampling temperature for assistant agent | `0.7` |
| `TRIAGE_BOT_OPENAI_MAX_TOKENS` | Maximum output tokens | `65536` |
| `TRIAGE_BOT_DISABLE_WEB_SERVER` | Disable the web server | `false` |
| `TRIAGE_BOT_SLACK_SIGNING_VERSION` | Slack signing version | Default in code |
| `TRIAGE_BOT_WEB_PORT` | Port for the web server | Default in code |
| `TRIAGE_BOT_TRACE_LEVEL` | Tracing level for logs | Default in code |

Each environment variable can also be specified in a `.hidden/config.toml` file:

```toml
openai_api_key = "your-api-key"
slack_app_token = "xapp-..."
slack_bot_token = "xoxb-..."
slack_signing_secret = "..."
db_endpoint = "http://localhost:8000"
db_username = "root"
db_password = "root"
```

Environment variables take precedence over values in the config file.

## Architecture and Extensibility

Triage-bot is designed with modularity and extensibility in mind, built around the following core components:

### Default Implementations

1. **Slack Integration** - The default chat client implementation connects to Slack using socket mode.

2. **SurrealDB Storage** - The default database client uses SurrealDB to store channel configurations, context, and message history.

3. **OpenAI Integration** - The LLM client uses OpenAI's API to generate responses and perform searches.

### Extensibility Through Traits

The application is structured around key traits that make it easy to extend or replace components:

- `GenericChatClient`: Interface for chat platform integration with methods for message handling.

- `GenericDbClient`: Interface for database operations, allowing alternative storage solutions.

- `GenericLlmClient`: Interface for LLM providers with methods for generating different types of responses.

To implement your own service integrations, simply create a new struct that implements the appropriate trait.

## Testing

```bash
$ cargo nextest run
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.