use thiserror::Error;
use url::ParseError;

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("create file error")]
    CreateFile(#[from] std::io::Error),
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
    #[error("reqwest error")]
    URIParse(#[from] ParseError),
    #[error("url invailable")]
    URI,
    #[error("download incomplete")]
    Incomplete,
    #[error("get content size error")]
    GetContentSize,
}
