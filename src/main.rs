use std::{env, error::Error, fmt};

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
  server-ops [--help]\n\
\n\
No server actions are performed yet.";

#[derive(Debug, PartialEq, Eq)]
enum AppError {
    InvalidArgument(String),
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
        }
    }
}

impl Error for AppError {}

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
        Some(argument) => Err(AppError::InvalidArgument(argument)),
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, HELP_TEXT, run};

    #[test]
    fn help_text_describes_the_utility() {
        assert!(HELP_TEXT.contains("updates available system packages"));
        assert!(HELP_TEXT.contains("server-ops [--help]"));
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
        assert_eq!(
            run([String::from("--unknown")]),
            Err(AppError::InvalidArgument(String::from("--unknown")))
        );
    }
}
