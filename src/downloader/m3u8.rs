use super::error::DownloadError;
use bytes::Buf;
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info, warn};
use m3u8_rs::{MasterPlaylist, MediaPlaylist, Playlist, KeyMethod};
use std::{collections::HashMap, vec};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::copy;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use url::Url;

use aes::cipher::{block_padding::Pkcs7, generic_array::GenericArray, BlockDecryptMut, KeyIvInit};

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

#[derive(Clone)]
struct AesKey {
    key: Vec<u8>,
    iv: Vec<u8>,
}

struct Segment {
    pub uri: String,
    pub save_file: String,
    pub try_count: i64,
    pub success: bool,
    pub key: Option<AesKey>,
}

impl Segment {
    pub fn new(uri: &str, save_file: &str, key: Option<AesKey>) -> Self {
        Self {
            uri: uri.to_string(),
            save_file: save_file.to_string(),
            try_count: 0,
            success: false,
            key,
        }
    }
}


// #[derive(Debug)]
pub struct M3U8Download {
    uri: String,
    save_file: String,
    cache_dir: String,
    segments: Vec<Segment>,
    try_count: i64,
    timeout: u64,
    cache_file: Option<String>,
    ignore_cache: bool,
    pbar: Option<ProgressBar>,
    climit: usize,
    aes_keys: HashMap<String, AesKey>,
}

impl M3U8Download {
    async fn download_segment(
        ts_uri: &str,
        save_file: &str,
        timeout: u64,
        key: Option<AesKey>,
    ) -> Result<(), DownloadError> {
        if let Some(key) = key {
            return Self::download_segment_with_key(ts_uri, save_file, timeout, key).await;
        }
        let mut file = File::create(save_file).await?;
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let request = client.get(ts_uri);
        let request = if timeout > 0 {
            request.timeout(std::time::Duration::from_secs(timeout))
        } else {
            request
        };
        let response = request.send().await?;
        copy(&mut response.bytes().await?.chunk(), &mut file).await?;
        Ok(())
    }

    async fn download_segment_with_key(
        ts_uri: &str,
        save_file: &str,
        timeout: u64,
        key: AesKey,
    ) -> Result<(), DownloadError> {
        let mut file = File::create(save_file).await?;
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let request = client.get(ts_uri);
        let request = if timeout > 0 {
            request.timeout(std::time::Duration::from_secs(timeout))
        } else {
            request
        };
        let response = request.send().await?;
        let bytes = response.bytes().await?.to_vec();
        let mut out_buf = vec![0u8; bytes.len()];

        let iv = GenericArray::from_slice(&key.iv);
        let key = GenericArray::from_slice(&key.key);

        let mut ct = Aes128CbcDec::new(key, iv)
            .decrypt_padded_b2b_mut::<Pkcs7>(bytes.as_slice(), out_buf.as_mut_slice()).unwrap();

        copy(&mut ct, &mut file).await?;
        Ok(())
    }

    fn join_path(&self, file: &str) -> String {
        std::path::Path::new(&self.cache_dir)
            .join(file)
            .to_str()
            .unwrap()
            .to_string()
    }

    async fn parse_media_playlist(
        &mut self,
        playlist: MediaPlaylist,
        base_url: &Url,
    ) -> Result<(), DownloadError> {
        for segment in playlist.segments {
            let key = match segment.key {
                Some(key) => {
                    match key.method {
                        KeyMethod::AES128 => {
                            let iv = key.iv.unwrap_or("0x00000000000000000000000000000000".to_string());
                            let iv = hex::decode(&iv.split("0x").last().unwrap()).unwrap();
                            let key_uri = base_url.join(key.uri.unwrap().as_str())?;
                            let key = self.get_aes_key(key_uri.as_str(), iv).await?;
                            Some(key)
                        },
                        _ => None,
                    }
                },
                None => None,
            };
            let segment_uri = base_url.join(segment.uri.as_str())?;
            let hash_name = sha256::digest(segment_uri.as_str());
            let segment = Segment::new(segment_uri.as_str(), &self.join_path(&hash_name), key);
            self.segments.push(segment);
        }
        Ok(())
    }

    async fn get_aes_key(&mut self, uri: &str, iv: Vec<u8>) -> Result<AesKey, DownloadError> {
        if let Some(key) = self.aes_keys.get(uri) {
            return Ok(key.clone());
        }
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let request = client.get(uri);
        let response = request.send().await?;
        let body = response.bytes().await?;
        let key = body.to_vec();
        let aes_key = AesKey { key, iv: iv.clone() };
        self.aes_keys.insert(uri.to_string(), aes_key.clone());
        Ok(aes_key)
    }

    async fn parse_master_playlist(
        &mut self,
        playlist: MasterPlaylist,
        base_url: &Url,
    ) -> Result<(), DownloadError> {
        let url = base_url.join(playlist.variants[0].uri.as_str())?;
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let body = client.get(url.as_str()).send().await?.bytes().await?;
        let (_i, playlist) =
            m3u8_rs::parse_media_playlist(&body).map_err(|_| DownloadError::URI)?;
        self.parse_media_playlist(playlist, &url).await
    }

    async fn parse_playlist(&mut self, base_url: &Url) -> Result<(), DownloadError> {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let body = client.get(base_url.as_str()).send().await?.bytes().await?;
        match m3u8_rs::parse_playlist(&body) {
            Result::Ok((_i, Playlist::MasterPlaylist(playlist))) => {
                self.parse_master_playlist(playlist, base_url).await
            }
            Result::Ok((_i, Playlist::MediaPlaylist(playlist))) => {
                self.parse_media_playlist(playlist, base_url).await
            }
            Result::Err(_) => Err(DownloadError::URI),
        }
    }

    async fn combine_files(&self, dst_file: &str) -> Result<(), DownloadError> {
        let mut output = File::create(dst_file).await?;
        for segment in self.segments.iter() {
            let mut input = File::open(&segment.save_file).await?;
            copy(&mut input, &mut output).await?;
        }
        info!("combine segments to {} success", dst_file);
        Ok(())
    }

    fn convert2mp4(&self, cache_file: &str) -> Result<(), DownloadError> {
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.arg("-i").arg(cache_file);
        cmd.arg("-c").arg("copy");
        cmd.arg(self.save_file.as_str());
        let output = cmd.output().expect("failed to execute process");
        if !output.status.success() {
            error!("convert to {} error", self.save_file);
            return Err(DownloadError::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("ffmpeg error: {}", String::from_utf8_lossy(&output.stderr)),
            )));
        }
        info!("convert to {} success", self.save_file);
        Ok(())
    }

    async fn convert(&mut self) -> Result<(), DownloadError> {
        let save_path = std::path::Path::new(&self.save_file);
        let extension = save_path.extension();
        if let Some(extension) = extension {
            if let Some(extension) = extension.to_str() {
                let hash_name = sha256::digest(&self.uri);
                let cache_file = self.join_path(&hash_name);
                match extension {
                    "mp4" => {
                        self.combine_files(&cache_file).await?;
                        self.convert2mp4(&cache_file)?;
                        self.cache_file.replace(cache_file);
                        return Ok(());
                    }
                    _ => return self.combine_files(&self.save_file).await,
                }
            }
        }
        self.combine_files(&self.save_file).await
    }

    fn check_integrity(&self) -> bool {
        for segment in self.segments.iter() {
            if !segment.success {
                return false;
            }
        }
        true
    }

    fn default_pbar(&self) -> ProgressBar {
        let sty = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:100.cyan/blue} {pos:>4}/{len:4} {msg}",
        )
        .unwrap();
        let pbar = ProgressBar::new(self.segments.len() as u64);
        pbar.set_style(sty);
        pbar.set_message(self.save_file.clone());
        pbar
    }

    pub async fn download(&mut self) -> Result<(), DownloadError> {
        std::fs::create_dir_all(&self.cache_dir)?;
        let url = Url::parse(self.uri.as_str())?;
        self.parse_playlist(&url).await?;

        if self.pbar.is_none() {
            self.pbar = Some(self.default_pbar());
        } else {
            self.pbar
                .as_mut()
                .unwrap()
                .set_length(self.segments.len() as u64);
        }

        let semaphore = Arc::new(Semaphore::new(self.climit));
        let mut tasks = JoinSet::new();

        for (index, segment) in self.segments.iter_mut().enumerate() {
            let meta = std::fs::metadata(&segment.save_file);
            let exists = if let Ok(meta) = meta {
                meta.len() > 0
            } else {
                false
            };
            if !exists || self.ignore_cache {
                let semaphore = semaphore.clone();
                let (uri, file, timeout, key) =
                    (segment.uri.clone(), segment.save_file.clone(), self.timeout, segment.key.clone());
                tasks.spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    (index, Self::download_segment(&uri, &file, timeout, key).await)
                });
            } else {
                info!("use cache file @ {} uri={}", index, segment.uri);
                segment.success = true;
                self.pbar.as_ref().unwrap().inc(1);
            }
        }

        while let Some(res) = tasks.join_next().await {
            if let Ok(result) = res {
                let (index, result) = result;
                if let Err(e) = result {
                    let segment = &mut self.segments[index];
                    warn!(
                        "download failed @ {} try_count={} err={} uri={}",
                        index, segment.try_count, e, segment.uri
                    );
                    if self.try_count < 0 || segment.try_count < self.try_count {
                        info!(
                            "try download @ {} try_count={} uri={}",
                            index, segment.try_count, segment.uri
                        );
                        let (uri, file, timeout , key) =
                            (segment.uri.clone(), segment.save_file.clone(), self.timeout, segment.key.clone());
                        tasks.spawn(async move {
                            (index, Self::download_segment(&uri, &file, timeout, key).await)
                        });
                        segment.try_count += 1;
                    } else {
                        error!(
                            "not download @ {} try_count={} uri={}",
                            index, self.try_count, segment.uri
                        );
                    }
                } else {
                    info!(
                        "download success @ {} uri={}",
                        index, self.segments[index].uri
                    );
                    self.segments[index].success = true;
                    self.pbar.as_ref().unwrap().inc(1);
                }
            } else {
                error!("download task error!");
            }
        }
        self.pbar.as_ref().unwrap().finish();
        self.convert().await?;
        if self.check_integrity() {
            Ok(())
        } else {
            Err(DownloadError::Incomplete)
        }
    }
}

impl Drop for M3U8Download {
    fn drop(&mut self) {
        for segment in self.segments.iter() {
            if !segment.success {
                if let Err(e) = std::fs::remove_file(&segment.save_file) {
                    warn!("remove segment tmp file {} err={}", segment.save_file, e);
                }
            }
        }
        if let Some(cache_file) = self.cache_file.as_ref() {
            if let Err(e) = std::fs::remove_file(cache_file) {
                warn!("remove tmp file {} err={}", cache_file, e);
            }
        }
    }
}

pub struct M3U8DownloadBuilder {
    uri: String,
    save_file: String,
    cache_dir: String,
    try_count: i64,
    timeout: u64,
    ignore_cache: bool,
    pbar: Option<ProgressBar>,
    climit: usize,
}

impl M3U8DownloadBuilder {
    pub fn new() -> Self {
        Self {
            uri: String::from(""),
            save_file: String::from(""),
            cache_dir: String::from(".cache"),
            try_count: -1,
            timeout: 0,
            ignore_cache: false,
            pbar: None,
            climit: 32,
        }
    }

    pub fn uri<T: Into<String>>(&mut self, uri: T) -> &mut Self {
        self.uri = uri.into();
        self
    }

    #[allow(unused)]
    pub fn cache_dir<T: Into<String>>(&mut self, dir: T) -> &mut Self {
        self.cache_dir = dir.into();
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
    pub fn climit(&mut self, limit: usize) -> &mut Self {
        self.climit = limit;
        self
    }

    #[allow(unused)]
    pub fn ignore_cache(&mut self, ignore: bool) -> &mut Self {
        self.ignore_cache = ignore;
        self
    }

    pub fn pbar(&mut self, pbar: ProgressBar) -> &mut Self {
        self.pbar.replace(pbar);
        self
    }

    pub fn build(&mut self) -> M3U8Download {
        M3U8Download {
            uri: self.uri.clone(),
            save_file: self.save_file.clone(),
            cache_dir: self.cache_dir.clone(),
            segments: Vec::new(),
            try_count: self.try_count,
            timeout: self.timeout,
            ignore_cache: self.ignore_cache,
            cache_file: None,
            pbar: self.pbar.take(),
            climit: self.climit,
            aes_keys: HashMap::new(),
        }
    }
}

#[tokio::test()]
async fn test_download() {
    let mut builder = M3U8DownloadBuilder::new();
    builder
        .uri("https://svip.high25-playback.com/20240922/7211_a45727d7/index.m3u8")
        .save_file("第08集.mp4")
        // .try_count(3)
        .timeout(5)
        .ignore_cache(true);
    let mut downloader = builder.build();
    downloader.download().await.unwrap();
}

#[tokio::test()]
async fn test_download_map() -> Result<(), DownloadError> {
    use indicatif::MultiProgress;
    use std::collections::HashMap;

    let videos: HashMap<&str, &str> = [
        (
            "第09集",
            "https://svip.high25-playback.com/20240928/8035_8dc312dd/index.m3u8",
        ),
        (
            "第10集",
            "https://svip.high25-playback.com/20240929/8117_a75b9603/index.m3u8",
        ),
    ]
    .iter()
    .cloned()
    .collect();

    let m = MultiProgress::new();
    let sty = ProgressStyle::with_template(
        "[{prefix}][{elapsed_precise}] {bar:100.cyan/blue} {pos:>4}/{len:4} {msg}",
    )
    .unwrap();
    // .progress_chars("##-");

    let mut builder = M3U8DownloadBuilder::new();
    let dir = "video";
    std::fs::create_dir_all(dir).unwrap();
    let mut index = 1;
    let len: usize = videos.len();
    for (key, value) in videos {
        let save_file = format!("{}/{}.mp4", dir, key);

        let pb = m.add(ProgressBar::hidden());
        pb.set_style(sty.clone());
        pb.set_message(save_file.clone());
        pb.set_prefix(format!("{}/{}", index, len));
        index += 1;

        let mut downloader = builder
            .uri(value)
            .timeout(10)
            .save_file(save_file)
            .ignore_cache(true)
            .pbar(pb)
            .build();
        downloader.download().await?
    }
    Ok(())
}
