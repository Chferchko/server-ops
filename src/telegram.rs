use std::{fmt, net::Ipv4Addr, time::Duration};

use serde::Serialize;

use crate::config::TelegramBotToken;

const TELEGRAM_API_BASE_URL: &str = "https://api.telegram.org/";

pub(crate) struct TelegramClient {
    http: reqwest::Client,
    endpoint: reqwest::Url,
}

impl TelegramClient {
    pub(crate) fn new(
        bot_token: &TelegramBotToken,
        timeout: Duration,
    ) -> Result<Self, TelegramClientError> {
        let mut endpoint =
            reqwest::Url::parse(TELEGRAM_API_BASE_URL).expect("the Telegram API base URL is valid");
        {
            let mut path = endpoint
                .path_segments_mut()
                .expect("the Telegram API base URL supports path segments");
            path.pop_if_empty();
            path.push(&format!("bot{}", bot_token.as_str()));
            path.push("sendMessage");
        }

        Self::with_endpoint(endpoint, timeout)
    }

    fn with_endpoint(
        endpoint: reqwest::Url,
        timeout: Duration,
    ) -> Result<Self, TelegramClientError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| TelegramClientError::ClientBuild)?;

        Ok(Self { http, endpoint })
    }

    pub(crate) async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
    ) -> Result<(), TelegramClientError> {
        let response = self
            .http
            .post(self.endpoint.clone())
            .json(&SendMessage { chat_id, text })
            .send()
            .await
            .map_err(|_| TelegramClientError::Request)?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(TelegramClientError::Response(response.status()))
        }
    }
}

#[derive(Debug)]
pub(crate) enum TelegramClientError {
    ClientBuild,
    Request,
    Response(reqwest::StatusCode),
}

impl fmt::Display for TelegramClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClientBuild => formatter.write_str("cannot create Telegram Bot API client"),
            Self::Request => formatter.write_str("Telegram Bot API request failed"),
            Self::Response(status) => {
                write!(formatter, "Telegram Bot API returned HTTP status {status}")
            }
        }
    }
}

impl std::error::Error for TelegramClientError {}

#[derive(Serialize)]
struct SendMessage<'a> {
    chat_id: i64,
    text: &'a str,
}

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

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::{ServerStageResult, TelegramClient, TelegramClientError, TelegramEvent};

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

    #[tokio::test]
    async fn sends_message_when_telegram_returns_success() {
        let (endpoint, server) = mock_server("HTTP/1.1 200 OK", r#"{"ok":true}"#).await;
        let client =
            TelegramClient::with_endpoint(endpoint, std::time::Duration::from_secs(1)).unwrap();

        client
            .send_message(42, "package update finished")
            .await
            .unwrap();

        let request = server.await.unwrap();
        assert!(request.starts_with("POST /bot-test/sendMessage HTTP/1.1\r\n"));
        assert!(request.contains("\r\ncontent-type: application/json\r\n"));
        assert!(request.ends_with(r#"{"chat_id":42,"text":"package update finished"}"#));
    }

    #[tokio::test]
    async fn returns_safe_error_when_telegram_returns_error() {
        let response_body = r#"{"ok":false,"description":"token must remain secret"}"#;
        let (endpoint, server) = mock_server("HTTP/1.1 401 Unauthorized", response_body).await;
        let client =
            TelegramClient::with_endpoint(endpoint, std::time::Duration::from_secs(1)).unwrap();

        let error = client
            .send_message(42, "package update finished")
            .await
            .unwrap_err();

        server.await.unwrap();
        assert!(matches!(
            error,
            TelegramClientError::Response(status) if status == reqwest::StatusCode::UNAUTHORIZED
        ));
        assert_eq!(
            error.to_string(),
            "Telegram Bot API returned HTTP status 401 Unauthorized"
        );
        assert!(!error.to_string().contains(response_body));
        assert!(!format!("{error:?}").contains(response_body));
    }

    async fn mock_server(
        status: &str,
        body: &str,
    ) -> (reqwest::Url, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let body = body.to_owned();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            let response = format!(
                "{status}\r\ncontent-length: {}\r\ncontent-type: application/json\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            request
        });
        let endpoint =
            reqwest::Url::parse(&format!("http://{address}/bot-test/sendMessage")).unwrap();

        (endpoint, server)
    }

    async fn read_request(stream: &mut tokio::net::TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0; 1024];

        loop {
            let bytes_read = stream.read(&mut buffer).await.unwrap();
            assert_ne!(bytes_read, 0, "mock server received an incomplete request");
            request.extend_from_slice(&buffer[..bytes_read]);

            if let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let headers = std::str::from_utf8(&request[..header_end]).unwrap();
                let content_length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if request.len() >= header_end + 4 + content_length {
                    return String::from_utf8(request).unwrap();
                }
            }
        }
    }
}
