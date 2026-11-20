# server-ops

`server-ops` will update available system packages on explicitly configured
Ubuntu and Debian servers and report the stages to Telegram. Remote operations
are not implemented yet.

## Dry run

Use `--dry-run <CONFIG_PATH>` to print the exact future SSH, `sudo apt update`,
and `sudo apt upgrade` stages for every configured server. It validates the
configuration but opens no SSH connection, runs no package command, and sends
no Telegram message.

```sh
cargo run -- --dry-run config.toml
```

## Configuration format

[`config.example.toml`](config.example.toml) is a public TOML template. Its IP
addresses are [RFC 5737](https://www.rfc-editor.org/rfc/rfc5737) documentation
addresses and its Telegram chat ID is deliberately invalid; neither identifies
a real server or chat.

Copy the template to `config.toml` and replace every placeholder before a future
run. Keep the completed file local: it contains your server inventory and chat
ID. `config.toml` is ignored by Git.

```sh
cp config.example.toml config.toml
```

The planned fields are:

| Section | Field | Meaning |
| --- | --- | --- |
| `run` | `dry_run` | Retained configuration marker for the future update workflow. Use the explicit `--dry-run` CLI mode to inspect planned work now. |
| `run` | `connection_timeout_seconds` | Maximum time for an SSH connection attempt. |
| `run` | `command_timeout_seconds` | Maximum time for one remote command. |
| `ssh` | `user` | Non-interactive SSH user, expected to have the required `sudo` permissions. |
| `ssh` | `private_key_path` | Local path to the private SSH key. Never commit this key. |
| `ssh` | `public_key_path` | Local path to the matching public SSH key. |
| `servers` | `address` | One explicitly approved IPv4 address per `[[servers]]` entry. |
| `telegram` | `chat_id` | Numeric destination chat ID. Replace the intentionally invalid `0`. |

The Telegram bot token is intentionally not a TOML field. Set it in the
`TELEGRAM_BOT_TOKEN` environment variable before running the utility; it must
not be committed to this file or to a tracked `.env` file.

Configuration loading and validation are not implemented yet; task-3.2 will
validate required fields, server addresses, duplicate servers, and the private
key file before any network operation.
