mod vrsr;
mod m3u8;
mod source;
use source::{FilmInfo, ZBKYYY, IJUJITV};
use source::SearchResult;

use m3u8::{DownloadError, M3U8DownloadBuilder};
use std::collections::HashMap;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};


async fn download(film: &impl FilmInfo) -> Result<(), DownloadError> {
    let pbars = MultiProgress::new();
    let sty = ProgressStyle::with_template(
        "[{prefix}][{elapsed_precise}] {bar:100.cyan/blue} {pos:>4}/{len:4} {msg}",
    )
    .unwrap();

    let base_dir = film.name();
    std::fs::create_dir_all(base_dir).unwrap();
    let mut builder = M3U8DownloadBuilder::new();
    let sources = film.sources();
    let source = &sources[0];
    for (index, uri) in source.iter().enumerate() {
        let save_file = format!("{}/第{:02}集.mp4", base_dir, index + 1);

        let pbar = pbars.add(ProgressBar::hidden());
        pbar.set_style(sty.clone());
        pbar.set_message(save_file.clone());
        pbar.set_prefix(format!("{:02}/{:02}", index + 1, source.len()));

        let mut cmd = std::process::Command::new("notify-send");
        cmd.arg("下载完成");
        cmd.arg(save_file.clone());
        cmd.arg("-t").arg("3000");

        let mut downloader = builder
            .uri(uri)
            .timeout(3)
            .save_file(save_file)
            // .try_count(3)
            .ignore_cache(true)
            .pbar(pbar)
            .build();
        downloader.download().await?;

        cmd.spawn().ok();
    }
    Ok(())
}

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

    Ok(())
}

// #[tokio::test()]
async fn test_vrsr() -> Result<(), vrsr::error::Error> {
    use vrsr::{RequestorBuilder, ZBKYYYParser, IJUJITVParser};
    use vrsr::create_resource;
    use vrsr::{Resource, Teleplay, Episode};

    let requestor = RequestorBuilder::new().build();
    let parser = ZBKYYYParser::new();
    // let parser = IJUJITVParser::new();

    let mut resource = create_resource(requestor, parser);
    let teleplays = resource.search("龙猫").await?;

    for teleplay in teleplays.iter() {
        let mut teleplay_locked  = teleplay.lock().await;
        println!("@@{}", teleplay_locked.title());
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
                    println!(">>{}", uri.uri);
                    let mut downloader = builder
                        .uri(uri.uri)
                        .timeout(3)
                        .save_file("test.mp4")
                        .ignore_cache(true)
                        .build();
                    downloader.download().await.unwrap();
                    break;
                }
            }
        }
    }
    
    Ok(())
}

#[tokio::test()]
async fn test_html() -> Result<(), reqwest::Error> {
    use scraper::{Html, Selector};
    use tokio::fs::File;
    use tokio::io::{copy, AsyncReadExt};

    // let url = "https://zbkyyy.com/qyvodsearch/-------------.html?wd=%E6%B5%B7%E8%B4%BC%E7%8E%8B";
    // let body = reqwest::get(url).await?.text().await?;
    // let mut file = File::create("output.html").await.unwrap();
    // copy(&mut body.as_bytes(), &mut file).await.unwrap();

    let mut file = File::open("output.html").await.unwrap();
    let mut body = String::new();
    file.read_to_string(&mut body).await.unwrap();

    let html = Html::parse_document(&body);

    let search_selector = Selector::parse("div.intro_con").unwrap();
    let score_selector = Selector::parse("div.tit span.s_score").unwrap();
    let name_selector = Selector::parse("div.tit span.s_tit a strong").unwrap();
    let type_selector = Selector::parse("div.tit span.s_type").unwrap();
    let url_selector = Selector::parse("div.tit span.s_tit a").unwrap();
    let films = html.select(&search_selector);
    for film in films {
        let fscore = film.select(&score_selector).next().unwrap();
        let ftype = film.select(&type_selector).next().unwrap();
        let fname = film.select(&name_selector).next().unwrap();
        let furl = film.select(&url_selector).next().unwrap();
        println!(
            "{} {} {} {}",
            fscore.inner_html(),
            ftype.inner_html(),
            fname.inner_html(),
            furl.value().attr("href").unwrap()
        );
    }
    Ok(())
}
