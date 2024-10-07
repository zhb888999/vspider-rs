mod args;
mod m3u8;
mod utils;
mod vrsr;
mod commands;

use args::{Cli, Mode};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use m3u8::{DownloadError, M3U8DownloadBuilder};
use std::collections::HashMap;
use commands::{CommandError, search, download, m3u8_download};


#[tokio::main]
async fn main() -> Result<(), CommandError> {
    env_logger::init();
    let cli = Cli::parse();
    if let Some(mode) = cli.mode {
        match mode {
            Mode::Search {keyword, src, all, nocache} => {
                search(&keyword, src, all, nocache).await?;
            },
            Mode::Download {id, src, index, nocache, save_dir, print, climit} => {
                download(id, src, index, nocache, &save_dir, print, climit).await?;
            }
            Mode::M3U8 {url, output, climit} => {
                m3u8_download(&url, &output, climit).await?;
            }
        }
    }

    // test_vrsr().await.unwrap();
    // test_vrsr_teleplay().await.unwrap();
    Ok(())
}

#[tokio::test()]
async fn test_vrsr() -> Result<(), vrsr::error::Error> {
    use vrsr::create_resource;
    use vrsr::{Episode, Resource, Teleplay};
    use vrsr::{IJUJITVParser, RequestorBuilder, ZBKYYYParser};

    let requestor = RequestorBuilder::new().build();
    let parser0 = ZBKYYYParser::new();
    let parser1 = IJUJITVParser::new();

    let mut resource0 = create_resource(requestor.clone(), parser0);
    let mut _resource1 = create_resource(requestor.clone(), parser1);
    let teleplays = resource0.search("探索新境").await?;

    for teleplay in teleplays.iter() {
        let mut teleplay_locked = teleplay.lock().await;
        let title = teleplay_locked.title().to_string();
        let base_dir = std::path::Path::new(&title);
        std::fs::create_dir_all(base_dir)?;
        let teleplay_src = teleplay_locked.request().await?;
        for episodes in teleplay_src.iter() {
            let mut tasks = tokio::task::JoinSet::new();
            for episode in episodes.iter() {
                let episode = episode.clone();
                tasks.spawn(async move { episode.lock().await.request().await });
            }
            let results = tasks.join_all().await;
            let mut builder = M3U8DownloadBuilder::new();
            for (result, episode) in results.iter().zip(episodes) {
                let episode_locked = episode.lock().await;
                let save_name = episode_locked.name();
                let mut save_path = base_dir.join(save_name);
                save_path.set_extension("mp4");
                if save_path.exists() {
                    continue;
                }
                if let Ok(uri) = result {
                    let mut downloader = builder
                        .uri(&uri.uri)
                        .timeout(3)
                        .save_file(save_path.to_str().unwrap())
                        .build();
                    downloader.download().await.unwrap();
                    // break;
                }
            }
            break;
        }
        // break;
    }

    Ok(())
}

#[tokio::test()]
async fn test_vrsr_teleplay() -> Result<(), vrsr::error::Error> {
    use vrsr::create_teleplay;
    use vrsr::{Episode, Teleplay};
    use vrsr::{IJUJITVParser, RequestorBuilder, ZBKYYYParser};

    let requestor = RequestorBuilder::new().build();

    let parser = ZBKYYYParser::new();
    // let parser  = IJUJITVParser::new();

    let mut teleplay = create_teleplay(requestor, parser, 96601);
    teleplay.request().await?;
    let title = teleplay.title().to_string();
    let teleplay_sr = teleplay.episodes();
    let base_dir = std::path::Path::new(&title);
    std::fs::create_dir_all(base_dir)?;
    for result in teleplay_sr.iter() {
        let mut tasks = tokio::task::JoinSet::new();
        for episode in result.iter() {
            let episode: std::sync::Arc<tokio::sync::Mutex<vrsr::BaseEpisode<vrsr::request::Requestor, ZBKYYYParser>>> = episode.clone();
            tasks.spawn(async move { episode.lock().await.request().await });
        }
        tasks.join_all().await;

        let mut builder = M3U8DownloadBuilder::new();
        for (index, episode) in result.iter().enumerate() {
            let save_file = base_dir.join(format!("第{:02}集.mp4", index + 1));
            if std::path::Path::exists(&save_file) {
                continue;
            }
            if index < 6 {
                continue;
            }
            let uri = episode.lock().await.uri();
            let mut downloader = builder
                .uri(uri.uri)
                .timeout(3)
                .save_file(save_file.to_string_lossy())
                .ignore_cache(true)
                .build();
            downloader.download().await.unwrap();
        }
    }

    Ok(())
}

#[allow(unused)]
async fn download_map(
    m3u8_map: &HashMap<String, String>,
    base_dir: &str,
) -> Result<(), DownloadError> {
    let pbars = MultiProgress::new();
    let sty = ProgressStyle::with_template(
        "[{prefix}][{elapsed_precise}] {bar:100.cyan/blue} {pos:>4}/{len:4} {msg}",
    )
    .unwrap();

    std::fs::create_dir_all(base_dir).unwrap();

    let mut builder = M3U8DownloadBuilder::new();
    for (index, (name, url)) in m3u8_map.iter().enumerate() {
        let save_file = format!("{}/{}.mp4", base_dir, name);
        let pbar = pbars.add(ProgressBar::hidden());
        pbar.set_style(sty.clone());
        pbar.set_message(save_file.clone());
        pbar.set_prefix(format!("{:02}/{:02}", index + 1, m3u8_map.len()));

        let mut downloader = builder
            .uri(url)
            .timeout(5)
            .save_file(save_file)
            .pbar(pbar)
            .build();
        downloader.download().await?;
    }
    Ok(())
}