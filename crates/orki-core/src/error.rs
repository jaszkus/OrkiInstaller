use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("config: {0}")]
    Config(String),
    #[error("pack: {0}")]
    Pack(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("unknown")]
    Unknown,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
