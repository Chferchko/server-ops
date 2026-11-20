use std::{
    collections::HashSet,
    error::Error,
    fmt, fs, io,
    net::Ipv4Addr,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) run: RunConfig,
    pub(crate) ssh: SshConfig,
    pub(crate) servers: Vec<ServerConfig>,
    pub(crate) telegram: TelegramConfig,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct RunConfig {
    pub(crate) dry_run: bool,
    pub(crate) connection_timeout_seconds: u64,
    pub(crate) command_timeout_seconds: u64,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct SshConfig {
    pub(crate) user: String,
    pub(crate) private_key_path: PathBuf,
    pub(crate) public_key_path: PathBuf,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct ServerConfig {
    pub(crate) address: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct TelegramConfig {
    pub(crate) chat_id: i64,
}

#[derive(Debug)]
pub(crate) enum ConfigError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    EmptyField(&'static str),
    InvalidServerAddress(String),
    DuplicateServer(Ipv4Addr),
    PrivateKeyNotFile(PathBuf),
    PrivateKeyUnavailable {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => {
                write!(formatter, "cannot read configuration `{}`", path.display())
            }
            Self::Parse { path, .. } => {
                write!(formatter, "cannot parse configuration `{}`", path.display())
            }
            Self::EmptyField(field) => {
                write!(formatter, "configuration field `{field}` must not be empty")
            }
            Self::InvalidServerAddress(address) => {
                write!(
                    formatter,
                    "server address `{address}` is not a valid IPv4 address"
                )
            }
            Self::DuplicateServer(address) => {
                write!(formatter, "server address `{address}` is duplicated")
            }
            Self::PrivateKeyNotFile(path) => {
                write!(
                    formatter,
                    "private key path `{}` is not a regular file",
                    path.display()
                )
            }
            Self::PrivateKeyUnavailable { path, .. } => {
                write!(
                    formatter,
                    "private key file `{}` is unavailable",
                    path.display()
                )
            }
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::PrivateKeyUnavailable { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::EmptyField(_)
            | Self::InvalidServerAddress(_)
            | Self::DuplicateServer(_)
            | Self::PrivateKeyNotFile(_) => None,
        }
    }
}

pub(crate) fn load(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let config = toml::from_str(&contents).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    validate(&config)?;
    Ok(config)
}

fn validate(config: &Config) -> Result<(), ConfigError> {
    require_text("ssh.user", &config.ssh.user)?;
    require_path("ssh.private_key_path", &config.ssh.private_key_path)?;
    require_path("ssh.public_key_path", &config.ssh.public_key_path)?;

    let mut addresses = HashSet::new();
    for server in &config.servers {
        let address = server
            .address
            .parse::<Ipv4Addr>()
            .map_err(|_| ConfigError::InvalidServerAddress(server.address.clone()))?;
        if !addresses.insert(address) {
            return Err(ConfigError::DuplicateServer(address));
        }
    }

    let metadata = fs::metadata(&config.ssh.private_key_path).map_err(|source| {
        ConfigError::PrivateKeyUnavailable {
            path: config.ssh.private_key_path.clone(),
            source,
        }
    })?;
    if !metadata.is_file() {
        return Err(ConfigError::PrivateKeyNotFile(
            config.ssh.private_key_path.clone(),
        ));
    }
    fs::File::open(&config.ssh.private_key_path).map_err(|source| {
        ConfigError::PrivateKeyUnavailable {
            path: config.ssh.private_key_path.clone(),
            source,
        }
    })?;

    Ok(())
}

fn require_text(field: &'static str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::EmptyField(field));
    }
    Ok(())
}

fn require_path(field: &'static str, path: &Path) -> Result<(), ConfigError> {
    if path.as_os_str().is_empty() {
        return Err(ConfigError::EmptyField(field));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::Ipv4Addr,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{ConfigError, load};

    static TEMP_DIRECTORY_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TestFiles {
        directory: PathBuf,
        private_key_path: PathBuf,
        config_path: PathBuf,
    }

    impl TestFiles {
        fn new() -> Self {
            let unique = TEMP_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "server-ops-config-test-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&directory).unwrap();

            let private_key_path = directory.join("id_ed25519");
            fs::write(&private_key_path, "test private key").unwrap();

            Self {
                config_path: directory.join("config.toml"),
                directory,
                private_key_path,
            }
        }

        fn write_config(&self, contents: &str) {
            fs::write(&self.config_path, contents).unwrap();
        }

        fn config(&self, servers: &str) -> String {
            format!(
                "[run]\n\
                 dry_run = true\n\
                 connection_timeout_seconds = 10\n\
                 command_timeout_seconds = 300\n\
                 \n\
                 [ssh]\n\
                 user = \"serverops\"\n\
                 private_key_path = \"{}\"\n\
                 public_key_path = \"/unused/public-key\"\n\
                 \n\
                 {servers}\n\
                 [telegram]\n\
                 chat_id = 1\n",
                self.private_key_path.display()
            )
        }
    }

    impl Drop for TestFiles {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.directory).unwrap();
        }
    }

    #[test]
    fn loads_valid_configuration() {
        let files = TestFiles::new();
        files.write_config(&files.config(
            "[[servers]]\naddress = \"192.0.2.10\"\n\n[[servers]]\naddress = \"198.51.100.20\"\n\n",
        ));

        let config = load(&files.config_path).unwrap();

        assert_eq!(config.ssh.user, "serverops");
        assert_eq!(config.servers.len(), 2);
        assert_eq!(
            config.servers[0].address.parse::<Ipv4Addr>().unwrap(),
            Ipv4Addr::new(192, 0, 2, 10)
        );
    }

    #[test]
    fn rejects_invalid_server_address() {
        let files = TestFiles::new();
        files.write_config(&files.config("[[servers]]\naddress = \"server.example\"\n\n"));

        assert!(matches!(
            load(&files.config_path),
            Err(ConfigError::InvalidServerAddress(address)) if address == "server.example"
        ));
    }

    #[test]
    fn rejects_duplicate_servers() {
        let files = TestFiles::new();
        files.write_config(&files.config(
            "[[servers]]\naddress = \"192.0.2.10\"\n\n[[servers]]\naddress = \"192.0.2.10\"\n\n",
        ));

        assert!(matches!(
            load(&files.config_path),
            Err(ConfigError::DuplicateServer(address)) if address == Ipv4Addr::new(192, 0, 2, 10)
        ));
    }

    #[test]
    fn rejects_missing_required_field() {
        let files = TestFiles::new();
        files.write_config(&files.config("[[servers]]\n\n"));

        assert!(matches!(
            load(&files.config_path),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn rejects_unavailable_private_key_file() {
        let files = TestFiles::new();
        let missing_key_path = files.directory.join("missing-key");
        let config = files
            .config("[[servers]]\naddress = \"192.0.2.10\"\n\n")
            .replace(
                &files.private_key_path.display().to_string(),
                &missing_key_path.display().to_string(),
            );
        files.write_config(&config);

        assert!(matches!(
            load(&files.config_path),
            Err(ConfigError::PrivateKeyUnavailable { path, .. }) if path == missing_key_path
        ));
    }

    #[test]
    fn rejects_empty_required_value() {
        let files = TestFiles::new();
        let config = files
            .config("[[servers]]\naddress = \"192.0.2.10\"\n\n")
            .replace("user = \"serverops\"", "user = \"\"");
        files.write_config(&config);

        assert!(matches!(
            load(&files.config_path),
            Err(ConfigError::EmptyField("ssh.user"))
        ));
    }
}
