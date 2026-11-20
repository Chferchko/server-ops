use std::{env, error::Error, fmt, future::Future, path::Path, time::Duration};

#[expect(
    dead_code,
    reason = "the command-line configuration path is scheduled for a later task"
)]
mod config;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Telegram event delivery is scheduled for a later task"
    )
)]
mod telegram;

const HELP_TEXT: &str = "server-ops updates available system packages on configured servers.\n\
\n\
Usage:\n\
  server-ops [--help | --dry-run <CONFIG_PATH> | --telegram-check <CONFIG_PATH>]\n\
\n\
Use --dry-run to inspect the planned server stages without network operations.";
const TELEGRAM_CHECK_MESSAGE: &str = "server-ops: Telegram delivery check.";
const TELEGRAM_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
enum AppError {
    InvalidArgument(String),
    Configuration(config::ConfigError),
    Telegram(telegram::TelegramClientError),
    Runtime(std::io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(argument) => {
                write!(
                    formatter,
                    "invalid argument `{argument}`. Run `server-ops --help` for usage."
                )
            }
            Self::Configuration(error) => error.fmt(formatter),
            Self::Telegram(error) => error.fmt(formatter),
            Self::Runtime(_) => formatter.write_str("cannot start the async runtime"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidArgument(_) => None,
            Self::Configuration(error) => Some(error),
            Self::Telegram(error) => Some(error),
            Self::Runtime(error) => Some(error),
        }
    }
}

fn main() {
    if let Err(error) = run(env::args().skip(1)) {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), AppError> {
    let mut arguments = arguments.into_iter();

    match arguments.next() {
        None => Ok(()),
        Some(argument) if argument == "--help" => match arguments.next() {
            None => {
                println!("{HELP_TEXT}");
                Ok(())
            }
            Some(argument) => Err(AppError::InvalidArgument(argument)),
        },
        Some(argument) if argument == "--telegram-check" => match arguments.next() {
            Some(config_path) => match arguments.next() {
                None => telegram_check(&config_path),
                Some(argument) => Err(AppError::InvalidArgument(argument)),
            },
            None => Err(AppError::InvalidArgument(argument)),
        },
        Some(argument) if argument == "--dry-run" => match arguments.next() {
            Some(config_path) => match arguments.next() {
                None => dry_run(&config_path),
                Some(argument) => Err(AppError::InvalidArgument(argument)),
            },
            None => Err(AppError::InvalidArgument(argument)),
        },
        Some(argument) => Err(AppError::InvalidArgument(argument)),
    }
}

fn dry_run(config_path: impl AsRef<Path>) -> Result<(), AppError> {
    let config = config::load(config_path).map_err(AppError::Configuration)?;
    print_dry_run_plan(&config, |line| println!("{line}"));
    Ok(())
}

fn print_dry_run_plan(config: &config::Config, mut write_line: impl FnMut(String)) {
    write_line(
        "Dry run: no SSH connections, package updates, or Telegram messages will be sent.".into(),
    );

    for server in &config.servers {
        write_line(format!("Server {}:", server.address));
        write_line(format!(
            "  1. Connect over SSH as {} using the configured key.",
            config.ssh.user
        ));
        write_line("  2. Run `sudo apt update`.".into());
        write_line("  3. Run `sudo apt upgrade`.".into());
    }
}

fn telegram_check(config_path: impl AsRef<Path>) -> Result<(), AppError> {
    let config = config::load(config_path).map_err(AppError::Configuration)?;
    let bot_token = config::load_telegram_bot_token().map_err(AppError::Configuration)?;
    let client = telegram::TelegramClient::new(&bot_token, TELEGRAM_REQUEST_TIMEOUT)
        .map_err(AppError::Telegram)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(AppError::Runtime)?;

    runtime
        .block_on(send_telegram_check(
            config.telegram.chat_id,
            |chat_id, message| client.send_message(chat_id, message),
        ))
        .map_err(AppError::Telegram)
}

async fn send_telegram_check<F, FutureResult, SendError>(
    chat_id: i64,
    send_message: F,
) -> Result<(), SendError>
where
    F: FnOnce(i64, &'static str) -> FutureResult,
    FutureResult: Future<Output = Result<(), SendError>>,
{
    send_message(chat_id, TELEGRAM_CHECK_MESSAGE).await
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, future, path::PathBuf, rc::Rc};

    use crate::config::{Config, RunConfig, ServerConfig, SshConfig, TelegramConfig};

    use super::{
        AppError, HELP_TEXT, TELEGRAM_CHECK_MESSAGE, print_dry_run_plan, run, send_telegram_check,
    };

    #[test]
    fn help_text_describes_the_utility() {
        assert!(HELP_TEXT.contains("updates available system packages"));
        assert!(HELP_TEXT.contains(
            "server-ops [--help | --dry-run <CONFIG_PATH> | --telegram-check <CONFIG_PATH>]"
        ));
    }

    #[test]
    fn invalid_argument_has_a_clear_error_message() {
        let error = run([String::from("--unknown")]).unwrap_err();

        assert_eq!(
            error.to_string(),
            "invalid argument `--unknown`. Run `server-ops --help` for usage."
        );
    }

    #[test]
    fn invalid_argument_is_returned_without_panicking() {
        assert!(matches!(
            run([String::from("--unknown")]),
            Err(AppError::InvalidArgument(argument)) if argument == "--unknown"
        ));
    }

    #[tokio::test]
    async fn telegram_check_sends_one_test_message_without_remote_operations() {
        let sent_messages = Rc::new(RefCell::new(Vec::new()));
        let sent_messages_for_sender = Rc::clone(&sent_messages);

        send_telegram_check(42, move |chat_id, message| {
            sent_messages_for_sender
                .borrow_mut()
                .push((chat_id, message.to_owned()));
            future::ready(Ok::<_, ()>(()))
        })
        .await
        .unwrap();

        assert_eq!(
            sent_messages.borrow().clone(),
            vec![(42, TELEGRAM_CHECK_MESSAGE.to_owned())]
        );
    }

    #[test]
    fn dry_run_lists_each_server_stage_without_network_calls() {
        let config = Config {
            run: RunConfig {
                dry_run: true,
                connection_timeout_seconds: 10,
                command_timeout_seconds: 300,
            },
            ssh: SshConfig {
                user: String::from("serverops"),
                private_key_path: PathBuf::from("/unused/id_ed25519"),
                public_key_path: PathBuf::from("/unused/id_ed25519.pub"),
            },
            servers: vec![
                ServerConfig {
                    address: String::from("192.0.2.10"),
                },
                ServerConfig {
                    address: String::from("198.51.100.20"),
                },
            ],
            telegram: TelegramConfig { chat_id: 42 },
        };
        let output = RefCell::new(Vec::new());

        print_dry_run_plan(&config, |line| output.borrow_mut().push(line));

        assert_eq!(
            output.into_inner(),
            vec![
                "Dry run: no SSH connections, package updates, or Telegram messages will be sent.",
                "Server 192.0.2.10:",
                "  1. Connect over SSH as serverops using the configured key.",
                "  2. Run `sudo apt update`.",
                "  3. Run `sudo apt upgrade`.",
                "Server 198.51.100.20:",
                "  1. Connect over SSH as serverops using the configured key.",
                "  2. Run `sudo apt update`.",
                "  3. Run `sudo apt upgrade`.",
            ]
        );
    }
}
