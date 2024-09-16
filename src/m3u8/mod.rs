use bytes::Buf;
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info, warn};
use m3u8_rs::{MasterPlaylist, MediaPlaylist, Playlist};
use thiserror::Error;
use tokio::fs::File;
use tokio::io::copy;
use tokio::task::JoinSet;
use url::{ParseError, Url};

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
    Incomplete
}

struct Segment {
    pub uri: String,
    pub save_file: String,
    pub try_count: i64,
    pub success: bool,
}

impl Segment {
    pub fn new(uri: &str, save_file: &str) -> Self {
        Self {
            uri: uri.to_string(),
            save_file: save_file.to_string(),
            try_count: 0,
            success: false,
        }
    }
}

// #[derive(Debug)]
pub struct M3U8Download {
    uri: String,
    save_file: String,
    tmp_dir: String,
    segments: Vec<Segment>,
    try_count: i64,
    timeout: u64,
    tmp_file: Option<String>,
    ignore_cache: bool,
    pbar: Option<ProgressBar>,
}

impl M3U8Download {
    async fn download_segment(
        ts_uri: &str,
        save_file: &str,
        timeout: u64,
    ) -> Result<(), DownloadError> {
        let mut file = File::create(save_file).await?;
        let client = reqwest::Client::new();
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

    fn join_path(&self, file: &str) -> String {
        std::path::Path::new(&self.tmp_dir)
            .join(file)
            .to_str()
            .unwrap()
            .to_string()
    }

    fn parse_media_playlist(
        &mut self,
        playlist: MediaPlaylist,
        base_url: &Url,
    ) -> Result<(), DownloadError> {
        for segment in playlist.segments {
            let segment_uri = base_url.join(segment.uri.as_str())?;
            let hash_name = sha256::digest(segment_uri.as_str());
            let segment = Segment::new(segment_uri.as_str(), &self.join_path(&hash_name));
            self.segments.push(segment);
        }
        Ok(())
    }

    async fn parse_master_playlist(
        &mut self,
        playlist: MasterPlaylist,
        base_url: &Url,
    ) -> Result<(), DownloadError> {
        let url = base_url.join(playlist.variants[0].uri.as_str())?;
        let body = reqwest::get(url.as_str()).await?.bytes().await?;
        let (_i, playlist) =
            m3u8_rs::parse_media_playlist(&body).map_err(|_| DownloadError::URI)?;
        self.parse_media_playlist(playlist, &url)
    }

    async fn parse_playlist(&mut self, base_url: &Url) -> Result<(), DownloadError> {
        let body = reqwest::get(base_url.as_str()).await?.bytes().await?;
        match m3u8_rs::parse_playlist(&body) {
            Result::Ok((_i, Playlist::MasterPlaylist(playlist))) => {
                self.parse_master_playlist(playlist, base_url).await
            }
            Result::Ok((_i, Playlist::MediaPlaylist(playlist))) => {
                self.parse_media_playlist(playlist, base_url)
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

    fn convert2mp4(&self, tmp_file: &str) -> Result<(), DownloadError> {
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.arg("-i").arg(tmp_file);
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
                let tmp_file = self.join_path(&hash_name);
                match extension {
                    "mp4" => {
                        self.combine_files(&tmp_file).await?;
                        self.convert2mp4(&tmp_file)?;
                        self.tmp_file.replace(tmp_file);
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
        std::fs::create_dir_all(&self.tmp_dir)?;
        let url = Url::parse(self.uri.as_str())?;
        self.parse_playlist(&url).await?;

        if self.pbar.is_none() {
            self.pbar = Some(self.default_pbar());
        } else {
            self.pbar.as_mut().unwrap().set_length(self.segments.len() as u64);
        }

        let mut tasks = JoinSet::new();

        for (index, segment) in self.segments.iter_mut().enumerate() {
            if !std::path::Path::new(&segment.save_file).exists() || self.ignore_cache {
                let (uri, file, timeout) =
                    (segment.uri.clone(), segment.save_file.clone(), self.timeout);
                tasks.spawn(
                    async move { (index, Self::download_segment(&uri, &file, timeout).await) },
                );
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
                        let (uri, file, timeout) =
                            (segment.uri.clone(), segment.save_file.clone(), self.timeout);
                        tasks.spawn(async move {
                            (index, Self::download_segment(&uri, &file, timeout).await)
                        });
                        segment.try_count += 1;
                    } else {
                        error!("not download @ {} try_count={} uri={}", index, self.try_count, segment.uri);
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
        if let Some(tmp_file) = self.tmp_file.as_ref() {
            if let Err(e) = std::fs::remove_file(tmp_file) {
                warn!("remove tmp file {} err={}", tmp_file, e);
            }
        }
    }
}

pub struct M3U8DownloadBuilder {
    uri: String,
    save_file: String,
    tmp_dir: String,
    try_count: i64,
    timeout: u64,
    ignore_cache: bool,
    pbar: Option<ProgressBar>,
}

impl M3U8DownloadBuilder {
    pub fn new() -> Self {
        Self {
            uri: String::from(""),
            save_file: String::from(""),
            tmp_dir: String::from(".tmp"),
            try_count: -1,
            timeout: 0,
            ignore_cache: false,
            pbar: None,
        }
    }

    pub fn uri<T: Into<String>>(&mut self, uri: T) -> &mut Self {
        self.uri = uri.into();
        self
    }

    #[allow(unused)]
    pub fn tmp_dir<T: Into<String>>(&mut self, dir: T) -> &mut Self {
        self.tmp_dir = dir.into();
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
            tmp_dir: self.tmp_dir.clone(),
            segments: Vec::new(),
            try_count: self.try_count,
            timeout: self.timeout,
            ignore_cache: self.ignore_cache,
            tmp_file: None,
            pbar: self.pbar.take(),
        }
    }
}


#[tokio::test()]
async fn test_download() {
    let mut builder = M3U8DownloadBuilder::new();
    builder.uri("https://bitdash-a.akamaihd.net/content/sintel/hls/playlist.m3u8")
        .save_file("test.mp4")
        .try_count(3)
        .timeout(5)
        .ignore_cache(true);
    let mut downloader = builder.build();
    downloader.download().await.unwrap();
}

#[tokio::test()]
async fn test_download_map() -> Result<(), DownloadError> {
    use std::collections::HashMap;
    use indicatif::MultiProgress;

    let videos: HashMap<&str, &str> = [
        ("第01集", "https://v.cdnlz2.com/20240409/29677_3f005fc9/index.m3u8"),
        ("第02集", "https://v.cdnlz2.com/20240409/29676_6e0be8f5/index.m3u8"),
        ("第03集", "https://v.cdnlz2.com/20240409/29675_31710f23/index.m3u8"),
        ("第04集", "https://v.cdnlz2.com/20240409/29674_bb521726/index.m3u8"),
        ("第05集", "https://v.cdnlz2.com/20240410/29726_2a457c22/index.m3u8"),
        ("第06集", "https://v.cdnlz2.com/20240410/29725_27347b37/index.m3u8"),
        ("第07集", "https://v.cdnlz2.com/20240411/29782_320d1815/index.m3u8"),
        ("第08集", "https://v.cdnlz2.com/20240411/29781_adebc99a/index.m3u8"),
        ("第09集", "https://v.cdnlz2.com/20240412/29838_d2bbc097/index.m3u8"),
        ("第10集", "https://v.cdnlz2.com/20240412/29837_8a2f86dc/index.m3u8"),
        ("第11集", "https://v.cdnlz2.com/20240413/29903_2b0e7f2a/index.m3u8"),
        ("第12集", "https://v.cdnlz2.com/20240413/29902_c21b5d65/index.m3u8"),
        ("第13集", "https://v.cdnlz2.com/20240414/29932_e942b904/index.m3u8"),
        ("第14集", "https://v.cdnlz2.com/20240414/29931_cc1baefb/index.m3u8"),
        ("第15集", "https://v.cdnlz2.com/20240415/29979_204f0e8e/index.m3u8"),
        ("第16集", "https://v.cdnlz2.com/20240415/29978_163ea09b/index.m3u8"),
        ("第17集", "https://v.cdnlz2.com/20240416/29995_f50d90d6/index.m3u8"),
        ("第18集", "https://v.cdnlz2.com/20240416/29994_e5576c71/index.m3u8"),
        ("第19集", "https://v.cdnlz2.com/20240417/30037_6d56ac2f/index.m3u8"),
        ("第20集", "https://v.cdnlz2.com/20240417/30038_f67bce37/index.m3u8"),
        ("第21集", "https://v.cdnlz2.com/20240418/30088_a4fa7307/index.m3u8"),
        ("第22集", "https://v.cdnlz2.com/20240418/30089_1c905630/index.m3u8"),
        ("第23集", "https://v.cdnlz2.com/20240419/30216_ec1c3a3a/index.m3u8"),
        ("第24集", "https://v.cdnlz2.com/20240419/30215_1aafce18/index.m3u8"),
        ("第25集", "https://v.cdnlz2.com/20240420/30253_7b2358d6/index.m3u8"),
        ("第26集", "https://v.cdnlz2.com/20240420/30252_43652b50/index.m3u8"),
        ("第27集", "https://v.cdnlz2.com/20240421/30320_a7f11884/index.m3u8"),
        ("第28集", "https://v.cdnlz2.com/20240421/30319_f36c7dd8/index.m3u8"),
        ("第29集", "https://v.cdnlz2.com/20240422/30365_a882004d/index.m3u8"),
        ("第30集", "https://v.cdnlz2.com/20240422/30364_e3f76fc7/index.m3u8"),
        ("第31集", "https://v.cdnlz2.com/20240423/30421_80576d76/index.m3u8"),
        ("第32集", "https://v.cdnlz2.com/20240423/30420_1923c90f/index.m3u8"),
        ("第33集", "https://v.cdnlz2.com/20240424/30475_5085748b/index.m3u8"),
    ].iter().cloned().collect();

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
    for (key , value) in videos {
        let save_file = format!("{}/{}.mp4",dir, key);

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
