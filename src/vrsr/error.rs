use scraper::error::SelectorErrorKind;

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
    #[error("Parser error: {0}")]
    ParseError(String),
    #[error("serde json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("browser error")]
    BrowserError,
}

impl<'a> From<SelectorErrorKind<'a>> for Error {
    fn from(e: SelectorErrorKind) -> Self {
        Error::ParseError(format!("Selector error: {}", e))
    }
}
