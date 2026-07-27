use thiserror::Error;

#[derive(Debug, Error)]
pub enum VideoError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("regex: {0}")]
    Regex(#[from] regex::Error),
    #[error("{0}")]
    Msg(String),
}
