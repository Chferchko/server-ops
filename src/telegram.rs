use std::net::Ipv4Addr;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TelegramEvent {
    RunStarted,
    ServerStageStarted {
        server: Ipv4Addr,
        stage: String,
    },
    ServerStageFinished {
        server: Ipv4Addr,
        stage: String,
        result: ServerStageResult,
    },
    RunSucceeded,
    RunFailed {
        error: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ServerStageResult {
    Succeeded,
    Failed(String),
}

impl TelegramEvent {
    pub(crate) fn format_message(&self) -> String {
        match self {
            Self::RunStarted => "server-ops: run started.".to_owned(),
            Self::ServerStageStarted { server, stage } => {
                format!("server-ops: server {server}: {stage} started.")
            }
            Self::ServerStageFinished {
                server,
                stage,
                result: ServerStageResult::Succeeded,
            } => {
                format!("server-ops: server {server}: {stage} succeeded.")
            }
            Self::ServerStageFinished {
                server,
                stage,
                result: ServerStageResult::Failed(error),
            } => {
                format!("server-ops: server {server}: {stage} failed: {error}")
            }
            Self::RunSucceeded => "server-ops: run succeeded.".to_owned(),
            Self::RunFailed { error } => format!("server-ops: run failed: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::{ServerStageResult, TelegramEvent};

    const SERVER: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 10);

    #[test]
    fn formats_run_start() {
        assert_eq!(
            TelegramEvent::RunStarted.format_message(),
            "server-ops: run started."
        );
    }

    #[test]
    fn formats_server_stage_start() {
        let event = TelegramEvent::ServerStageStarted {
            server: SERVER,
            stage: String::from("apt update"),
        };

        assert_eq!(
            event.format_message(),
            "server-ops: server 192.0.2.10: apt update started."
        );
    }

    #[test]
    fn formats_successful_server_stage() {
        let event = TelegramEvent::ServerStageFinished {
            server: SERVER,
            stage: String::from("apt upgrade"),
            result: ServerStageResult::Succeeded,
        };

        assert_eq!(
            event.format_message(),
            "server-ops: server 192.0.2.10: apt upgrade succeeded."
        );
    }

    #[test]
    fn formats_failed_server_stage() {
        let event = TelegramEvent::ServerStageFinished {
            server: SERVER,
            stage: String::from("apt update"),
            result: ServerStageResult::Failed(String::from("connection timed out")),
        };

        assert_eq!(
            event.format_message(),
            "server-ops: server 192.0.2.10: apt update failed: connection timed out"
        );
    }

    #[test]
    fn formats_run_success() {
        assert_eq!(
            TelegramEvent::RunSucceeded.format_message(),
            "server-ops: run succeeded."
        );
    }

    #[test]
    fn formats_run_failure() {
        let event = TelegramEvent::RunFailed {
            error: String::from("configuration is unavailable"),
        };

        assert_eq!(
            event.format_message(),
            "server-ops: run failed: configuration is unavailable"
        );
    }
}
