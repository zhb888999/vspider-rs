mod vrsr;
mod m3u8;

use m3u8::{DownloadError, M3U8DownloadBuilder};
use vrsr::{GenerateInfo, ResourceParse, TeleplayParse, EpisodeParse};
use std::collections::HashMap;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};


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

#[tokio::main]
async fn main() -> Result<(), DownloadError> {
    env_logger::init();
    // let mut film = IJUJITV::from_id(5139);
    // film.parse().await?;
    // download(&film).await?;
    // let mut result = ZBKYYY::search("海贼王").await?;
    // for film in result.iter_mut() {
    //     film.film.parse().await?;
    //     println!("{}", film.film.name());
    //     download(&film.film).await?;
    // }
    test_vrsr().await.unwrap();
    // test_vrsr_teleplay().await.unwrap();
    Ok(())
}

// #[tokio::test()]
async fn test_vrsr() -> Result<(), vrsr::error::Error> {
    use vrsr::{RequestorBuilder, ZBKYYYParser, IJUJITVParser};
    use vrsr::create_resource;
    use vrsr::{Resource, Teleplay, Episode};

    let requestor = RequestorBuilder::new().build();
    // let parser = ZBKYYYParser::new();
    let parser = IJUJITVParser::new();

    let mut resource = create_resource(requestor, parser);
    let teleplays = resource.search("庆余年").await?;

    for teleplay in teleplays.iter() {
        let mut teleplay_locked  = teleplay.lock().await;
        let teleplay_sr = teleplay_locked.request().await?;
        for result in teleplay_sr.iter() {
            let mut tasks = tokio::task::JoinSet::new();
            for episode in result.iter() {
                let episode = episode.clone();
                tasks.spawn( async move {
                    episode.lock().await.request().await
                });
            }
            let results = tasks.join_all().await;
            let mut builder = M3U8DownloadBuilder::new();
            for res in results {
                if let Ok(uri) = res {
                    // println!(">>{}", uri.uri);
                    // let mut downloader = builder
                    //     .uri(uri.uri)
                    //     .timeout(3)
                    //     .save_file("test.mp4")
                    //     .build();
                    // downloader.download().await.unwrap();
                    // break;
                }
            }
        }
    }
    
    Ok(())
}

async fn test_vrsr_teleplay() -> Result<(), vrsr::error::Error> {
    use vrsr::{RequestorBuilder, ZBKYYYParser, IJUJITVParser};
    use vrsr::create_teleplay;
    use vrsr::{Teleplay, Episode};

    let requestor = RequestorBuilder::new().build();

    let parser  = ZBKYYYParser::new();
    // let parser  = IJUJITVParser::new();

    let mut teleplay = create_teleplay(requestor, parser, "探索新境", 96601);
    let title = teleplay.title().to_string();
    let base_dir = std::path::Path::new(&title);
    std::fs::create_dir_all(base_dir)?;
    let teleplay_sr = teleplay.request().await?;
    for result in teleplay_sr.iter() {
        let mut tasks = tokio::task::JoinSet::new();
        for episode in result.iter() {
            let episode = episode.clone();
            tasks.spawn( async move {
                episode.lock().await.request().await
            });
        }
        tasks.join_all().await;

        let mut builder = M3U8DownloadBuilder::new();
        for (index,episode) in result.iter().enumerate() {
            let save_file = base_dir.join(format!("第{:02}集.mp4", index + 1));
            if std::path::Path::exists(&save_file) { continue; }
            if index < 6 { continue; }
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
