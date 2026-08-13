use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("http error: {0}")] Http(#[from] reqwest::Error),

    #[error("io error: {0}")] Io(#[from] std::io::Error),

    #[error("unexpected status code {0}")] UnexpectedStatus(u16),

    #[error("other: {0}")] Other(String),
}
