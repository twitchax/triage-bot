[![Build and Test](https://github.com/twitchax/triage-bot/actions/workflows/build.yml/badge.svg)](https://github.com/twitchax/triage-bot/actions/workflows/build.yml)
[![codecov](https://codecov.io/gh/twitchax/triage-bot/branch/main/graph/badge.svg?token=35MZN0YFZF)](https://codecov.io/gh/twitchax/triage-bot)
[![Version](https://img.shields.io/crates/v/triage-bot.svg)](https://crates.io/crates/triage-bot)
[![Crates.io](https://img.shields.io/crates/d/triage-bot?label=crate)](https://crates.io/crates/triage-bot)
[![GitHub all releases](https://img.shields.io/github/downloads/twitchax/triage-bot/total?label=binary)](https://github.com/twitchax/triage-bot/releases)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# triage-bot

An OpenAI-powered triage bot for a slack support channel designed to tag oncalls, prioritize issues, suggest solutions, and streamline communication.

## Usage

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

## Testing

```bash
$ cargo nextest run
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.