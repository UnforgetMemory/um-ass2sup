use thiserror::Error;

#[derive(Error, Debug)]
pub enum BdnError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XML generation error: {0}")]
    Xml(String),

    #[error("PNG encoding error: {0}")]
    Png(String),

    #[error("Invalid frame data: {0}")]
    InvalidFrame(String),
}
