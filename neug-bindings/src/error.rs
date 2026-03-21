use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum Error {
    #[error("Database is closed")]
    DatabaseClosed,

    #[error("Connection is closed")]
    ConnectionClosed,

    #[error("Execution error: {0}")]
    ExecutionFailed(String),

    #[error("Initialization error: {0}")]
    InitializationFailed(String),

    #[error("Invalid arguments: {0}")]
    InvalidArgument(String),
}

pub type Result<T> = std::result::Result<T, Error>;
