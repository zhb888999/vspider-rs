
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Request out of try: {0}")]
    RequestOutOfTry(u64),
    #[error("Response failed")]
    ResponseFailed(u16),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("parse error: {0}")]
    ParseError(String)
}
