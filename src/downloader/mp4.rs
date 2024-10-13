use super::error::DownloadError;
use futures::stream::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

pub struct MP4Download {
    uri: String,
    save_file: String,
    try_count: i64,
    timeout: u64,
    pbar: Option<ProgressBar>,
}

impl MP4Download {
    fn byte2mb(size: u64) -> u64 {
        size / 1024 / 1024
    }

    async fn get_total_size(&self) -> Result<Option<u64>, DownloadError> {
        let client = reqwest::Client::new();
        let total_size = {
            let resp = client.head(&self.uri).send().await?;
            if resp.status().is_success() {
                resp.headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|ct_len| ct_len.to_str().ok())
                    .and_then(|ct_len| ct_len.parse().ok())
            } else {
                return Err(DownloadError::GetContentSize);
            }
        };
        Ok(total_size)
    }

    fn default_pbar(&self, total_size: u64) -> ProgressBar {
        let sty = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:100.cyan/blue} {pos:>4}/{len:4}MB {msg}",
        )
        .unwrap();
        let pbar = ProgressBar::new(Self::byte2mb(total_size));
        pbar.set_style(sty);
        pbar.set_message(self.save_file.clone());
        pbar
    }

    async fn download_task(&mut self, total_size: u64) -> Result<(), DownloadError> {
        let path = std::path::Path::new(&self.save_file);
        if path.exists() {
            std::fs::remove_file(path).unwrap();
        }
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let request = client.get(&self.uri);
        let request = if self.timeout > 0 {
            request.timeout(std::time::Duration::from_secs(self.timeout))
        } else {
            request
        };

        let mut download_size = 0u64;

        let source = request.send().await?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        let mut stream = source.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            download_size += chunk.len() as u64;
            self.pbar
                .as_ref()
                .unwrap()
                .set_position(Self::byte2mb(download_size));
        }
        self.pbar
            .as_ref()
            .unwrap()
            .set_position(Self::byte2mb(total_size));
        self.pbar.as_ref().unwrap().finish();
        Ok(())
    }

    pub async fn download(&mut self) -> Result<(), DownloadError> {
        let total_size = self.get_total_size().await?.unwrap_or(0);
        if self.pbar.is_none() {
            self.pbar = Some(self.default_pbar(total_size));
        } else {
            self.pbar.as_mut().unwrap().set_length(total_size);
        }
        let mut try_count = 0i64;
        loop {
            match self.download_task(total_size).await {
                Ok(_) => {
                    break;
                }
                Err(err) => match err {
                    DownloadError::Reqwest(_) => {
                        if self.try_count < 0 || try_count < self.try_count {
                            try_count += 1;
                            continue;
                        }
                        return Err(err);
                    }
                    _ => {
                        return Err(err);
                    }
                },
            }
        }
        Ok(())
    }
}

pub struct MP4DownloadBuilder {
    uri: String,
    save_file: String,
    try_count: i64,
    timeout: u64,
    pbar: Option<ProgressBar>,
}

impl MP4DownloadBuilder {
    pub fn new() -> Self {
        Self {
            uri: String::from(""),
            save_file: String::from(""),
            try_count: -1,
            timeout: 0,
            pbar: None,
        }
    }

    pub fn uri<T: Into<String>>(&mut self, uri: T) -> &mut Self {
        self.uri = uri.into();
        self
    }

    pub fn save_file<T: Into<String>>(&mut self, save: T) -> &mut Self {
        self.save_file = save.into();
        self
    }

    #[allow(unused)]
    pub fn try_count(&mut self, count: i64) -> &mut Self {
        self.try_count = count;
        self
    }

    #[allow(unused)]
    pub fn timeout(&mut self, second: u64) -> &mut Self {
        self.timeout = second;
        self
    }

    #[allow(unused)]
    pub fn pbar(&mut self, pbar: ProgressBar) -> &mut Self {
        self.pbar.replace(pbar);
        self
    }

    pub fn build(&mut self) -> MP4Download {
        MP4Download {
            uri: self.uri.clone(),
            save_file: self.save_file.clone(),
            try_count: self.try_count,
            timeout: self.timeout,
            pbar: self.pbar.take(),
        }
    }
}
