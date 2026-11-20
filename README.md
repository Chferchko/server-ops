# server-ops

`server-ops` will update available system packages on explicitly configured
Ubuntu and Debian servers and report the stages to Telegram. Remote operations
are not implemented yet.

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
| `run` | `dry_run` | Leave `true` while inspecting planned work; dry-run behavior will be added in task-5.3. |
| `run` | `connection_timeout_seconds` | Maximum time for an SSH connection attempt. |
| `run` | `command_timeout_seconds` | Maximum time for one remote command. |
| `ssh` | `user` | Non-interactive SSH user, expected to have the required `sudo` permissions. |
| `ssh` | `private_key_path` | Local path to the private SSH key. Never commit this key. |
| `ssh` | `public_key_path` | Local path to the matching public SSH key. |
| `servers` | `address` | One explicitly approved IPv4 address per `[[servers]]` entry. |
| `telegram` | `chat_id` | Numeric destination chat ID. Replace the intentionally invalid `0`. |

The Telegram bot token is intentionally not a TOML field. It will be read from
an environment variable in task-3.3, so it must not be committed to this file
or to a `.env` file that is tracked by Git.

Configuration loading and validation are not implemented yet; task-3.2 will
validate required fields, server addresses, duplicate servers, and the private
key file before any network operation.
