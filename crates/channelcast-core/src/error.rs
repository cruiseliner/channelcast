use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("config: {0}")]
    Config(String),
    #[error("toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("feed parse: {0}")]
    Feed(String),
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("channel not found: {0}")]
    ChannelNotFound(String),
    #[error("channel {0} has no feeds configured (stub)")]
    StubChannel(String),
}

pub type Result<T> = std::result::Result<T, Error>;
