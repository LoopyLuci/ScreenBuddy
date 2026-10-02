use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Creature not found: {0}")]
    CreatureNotFound(String),
    #[error("Invalid definition: {0}")]
    InvalidDefinition(String),
    #[error("Validation failed: {0:?}")]
    Validation(Vec<String>),
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Render error: {0}")]
    Render(String),
    #[error("Other: {0}")]
    Other(String),
}
