use super::error::Error;
use super::Request;
use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONNECTION, CONTENT_TYPE,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{copy, AsyncReadExt};

#[derive(Debug, Clone)]
pub struct Requestor {
    headers: HeaderMap,
    cache_dir: String,
    timeout: u64,
    try_count: u64,
    client: reqwest::Client,
    ignore_cache: bool,
}

impl Requestor {
    async fn base_request(&self, url: &str) -> Result<String, Error> {
        let response = self
            .client
            .clone()
            .get(url)
            .headers(self.headers.clone())
            .timeout(std::time::Duration::from_secs(self.timeout))
            .send()
            .await?;

        if response.status().is_success() {
            let body = response.text().await?;
            return Ok(body);
        } else {
            return Err(Error::ResponseFailed(response.status().as_u16()));
        }
    }

    fn get_cache_path(&self, url: &str) -> String {
        let hash_name = sha256::digest(url);
        std::path::Path::new(&self.cache_dir)
            .join(hash_name)
            .to_str()
            .unwrap()
            .to_string()
    }

    async fn write_cache(&self, path: &str, content: &str) -> Result<(), std::io::Error> {
        let mut file = tokio::fs::File::create(path).await?;
        copy(&mut content.as_bytes(), &mut file).await?;
        Ok(())
    }

    async fn read_cache(&self, path: &str) -> Result<String, std::io::Error> {
        if std::path::Path::new(&path).exists() {
            let mut file = tokio::fs::File::open(path).await?;
            let mut content = String::new();
            file.read_to_string(&mut content).await?;
            Ok(content)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "cache not found",
            ))
        }
    }

    async fn modifie_time(&self, path: &str) -> Option<Duration> {
        if std::path::Path::new(&path).exists() {
            let metadata = std::fs::metadata(path).unwrap();
            let modified_time = metadata.modified().unwrap();
            let current_time = std::time::SystemTime::now();
            let duration_since_modified = current_time.duration_since(modified_time);
            if let Ok(duration) = duration_since_modified {
                return Some(duration);
            } else {
                println!("get modifie time error, path: {}", path);
            }
        }
        None
    }
}

impl Request for Requestor {
    async fn request(&self, url: &str) -> Result<String, Error> {
        let mut try_count = 0u64;
        while self.try_count == 0 || try_count < self.try_count {
            match self.base_request(url).await {
                Ok(content) => return Ok(content),
                Err(e) => {
                    if let Error::ResponseFailed(status) = e {
                        return Err(Error::ResponseFailed(status));
                    }
                }
            }
            try_count += 1;
        }
        Err(Error::RequestOutOfTry(try_count))
    }

    async fn post_request(
        &self,
        url: &str,
        form_data: HashMap<String, String>,
    ) -> Result<String, Error> {
        let mut try_count = 0u64;
        while self.try_count == 0 || try_count < self.try_count {
            let response = self
                .client
                .clone()
                .post(url)
                .form(&form_data)
                .headers(self.headers.clone())
                .send()
                .await?;
            if response.status().is_success() {
                let body = response.text().await?;
                return Ok(body);
            }
            try_count += 1;
        }
        Err(Error::RequestOutOfTry(try_count))
    }

    async fn request_with_cache(&self, url: &str, cache_time: Duration) -> Result<String, Error> {
        let cache_path = self.get_cache_path(url);
        if !self.ignore_cache {
            if let Some(time) = self.modifie_time(&cache_path).await {
                if time < cache_time {
                    let cache = self.read_cache(&cache_path).await;
                    if let Ok(cache) = cache {
                        return Ok(cache);
                    } else {
                        println!("read cache error, request url: {}", url);
                    }
                }
            }
        }
        let content = self.request(url).await?;
        if let Err(e) = self.write_cache(&cache_path, &content).await {
            println!("write cache error, url: {}, error: {}", url, e);
        }
        Ok(content)
    }
}

pub struct RequestorBuilder {
    headers: HeaderMap,
    cache_dir: String,
    timeout: u64,
    try_count: u64,
    client: reqwest::Client,
    ignore_cache: bool,
}

impl Default for RequestorBuilder {
    fn default() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/html"));
        headers.insert(
            "User-Agent",
            HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36"),
        );
        headers.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static("gzip, deflate, br"),
        );
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,zh;q=0.8,en;q=0.6"),
        );
        headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        Self {
            headers,
            cache_dir: String::from(".cache"),
            timeout: 30,
            try_count: 3,
            client,
            ignore_cache: false,
        }
    }
}

impl RequestorBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(unused)]
    pub fn headers(&mut self, headers: HeaderMap) -> &mut Self {
        self.headers = headers;
        self
    }

    #[allow(unused)]
    pub fn cache_dir(&mut self, cache_dir: &str) -> &mut Self {
        self.cache_dir = cache_dir.to_string();
        self
    }

    #[allow(unused)]
    pub fn timeout(&mut self, timeout: u64) -> &mut Self {
        self.timeout = timeout;
        self
    }

    #[allow(unused)]
    pub fn try_count(&mut self, count: u64) -> &mut Self {
        self.try_count = count;
        self
    }

    pub fn ignore_cache(&mut self, ignore: bool) -> &mut Self {
        self.ignore_cache = ignore;
        self
    }

    pub fn build(&self) -> Arc<Requestor> {
        std::fs::create_dir_all(&self.cache_dir).unwrap();
        Arc::new(Requestor {
            headers: self.headers.clone(),
            cache_dir: self.cache_dir.clone(),
            timeout: self.timeout,
            try_count: self.try_count,
            client: self.client.clone(),
            ignore_cache: self.ignore_cache,
        })
    }
}
