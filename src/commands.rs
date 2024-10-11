use crate::vrsr::error::Error as VRSRError;
use crate::m3u8::{DownloadError, M3U8DownloadBuilder};
use crate::vrsr::{create_resource, create_teleplay, Episode, GeneralTeleplay, RequestorBuilder, Resource, Teleplay, URIType};
use crate::vrsr::{Request, GenerateInfo, ResourceParse, TeleplayParse, EpisodeParse};
use crate::vrsr::ZBKYYYParser;
use crate::vrsr::IJUJITVParser;
use crate::vrsr::JUGOUGOUParser;
use bytes::Buf;
use tokio::io::{copy, AsyncWriteExt};
use thiserror::Error;
use crate::args::Src;
use crate::vrsr::GeneralResource;
use anyhow::anyhow;
use tokio::fs;
use futures::stream::{self, StreamExt};


pub async fn download_file(url: &str, save_path: &str) -> Result<(), anyhow::Error> {
    let path = std::path::Path::new(&save_path);
    let client = reqwest::Client::new();
    let total_size = {
        let resp = client.head(url).send().await?;
        if resp.status().is_success() {
            resp.headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|ct_len| ct_len.to_str().ok())
                .and_then(|ct_len| ct_len.parse().ok())
                .unwrap_or(0)
        } else {
            return Err(anyhow!(
                "Couldn't download URL: {}. Error: {:?}",
                url,
                resp.status(),
            ));
        }
    };
    let client = reqwest::Client::new();
    let mut request = client.get(url);
    let pb = indicatif::ProgressBar::new(total_size / 1024 / 1024);
    let sty = indicatif::ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:100.cyan/blue} {pos:>4}/{len:4}MB {msg}",
    ).unwrap();
    pb.set_style(sty);
    pb.set_message(save_path.to_string());

    if path.exists() {
        let size = path.metadata()?.len().saturating_sub(1);
        request = request.header(reqwest::header::RANGE, format!("bytes={}-", size));
        pb.inc(size);
    }
    let mut download_size = 0u64;
    let source = request.send().await?;
    let mut dest = fs::OpenOptions::new().create(true).append(true).open(&path).await?;
    let mut stream = source.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        dest.write_all(&chunk).await?;
        download_size += chunk.len() as u64;
        pb.set_position(download_size / 1024 / 1024);
    }
    pb.set_position(total_size / 1024 / 1024);
    pb.finish_with_message(format!("{} succcess", save_path));
    Ok(())
}

async fn search_resource<'a, R, P>(mut resource: GeneralResource<'a, R, P>, arg_value: &str, keyword: &str) -> Result<(), VRSRError> 
where
    R: Request,
    P: GenerateInfo + ResourceParse + TeleplayParse + EpisodeParse,

{
    println!("@:{}[{}]", arg_value, resource.name().to_string());
    let teleplays = resource.search(keyword).await?;
    for teleplay in teleplays.iter() {
        let teleplay_locked = teleplay.lock().await;
        println!("{}", teleplay_locked.info());
        println!("==============");
    }
    Ok(())
}

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Parser resource error: {0:?}")]
    ParserResourceError(#[from] VRSRError),
    #[error("M3U8 download error: {0:?}")]
    M3U8DownloadError(#[from] DownloadError),
}

pub async fn search(keyword: &str, src: Src, all: bool, nocache: bool) -> Result<(), CommandError> {
    let requestor = RequestorBuilder::new()
        .ignore_cache(nocache)
        .build();
    if all {search_resource(create_resource(requestor.clone(), ZBKYYYParser::new()), "zbkyyy", keyword).await?;
        search_resource(create_resource(requestor.clone(), IJUJITVParser::new()), "ijujitv", keyword).await?;
        search_resource(create_resource(requestor.clone(), JUGOUGOUParser::new()), "jugougou", keyword).await?;
    } else {
        match src {
            Src::ZBKYYY => search_resource(create_resource(requestor.clone(), ZBKYYYParser::new()), "zbkyyy", keyword).await?,
            Src::IJUJITV => search_resource(create_resource(requestor.clone(), IJUJITVParser::new()), "ijujitv", keyword).await?,
            Src::JUGOUGOU => search_resource(create_resource(requestor.clone(), JUGOUGOUParser::new()), "jugougou", keyword).await?,
        }
    }
    Ok(())
}

async fn dwonload_teleplay<'a, R, P>(mut teleplay: GeneralTeleplay<R, P>, index: usize, save_dir: &Option<String>, print: bool, climit: usize) -> Result<(), VRSRError> 
where
    R: Request,
    P: TeleplayParse + EpisodeParse,

{
    teleplay.request().await?;
    let save_path = if let Some(save_dir) = save_dir {
        std::path::Path::new(save_dir)
    } else {
        std::path::Path::new(teleplay.title())
    };
    println!("===> {}", teleplay.title());
    if !save_path.exists() {
        std::fs::create_dir_all(save_path)?;
    }
    let teleplay_src = teleplay.episodes();

    if print {
        for (index, result) in teleplay_src.iter().enumerate() {
            println!("{} -> {}", index + 1, result.0.as_ref().unwrap_or(&"unknown".to_string()));
            for episode in result.1.iter() {
                let episode_locked = episode.lock().await;
                print!("[{}]", episode_locked.name());
            }
            println!();
        }
    } else {
        if let Some(result) = teleplay_src.get(index - 1) {
            let mut builder = M3U8DownloadBuilder::new();
            for (index, episode) in result.1.iter().enumerate() {
                let mut episode_locked = episode.lock().await;
                println!("download {} => {}", index, episode_locked.name());
                let save_file = save_path.join(format!("{}.mp4", episode_locked.name()));
                if std::path::Path::exists(&save_file) {
                    continue;
                }
                let uri = episode_locked.request().await?;
                let save_file = save_file.to_string_lossy();
                match uri.utype {
                    URIType::M3U8 => {
                        let mut downloader = builder
                            .uri(uri.uri)
                            .timeout(3)
                            .climit(climit)
                            .save_file(save_file)
                            .build();
                        downloader.download().await.unwrap();
                    }
                    URIType::MP4 => {
                        download_file(&uri.uri,&save_file).await.unwrap();
                    }
                    _ => {
                        println!("Unsupported URI type");
                    }
                }
            }
        } else {
            println!("No such episode");
        }
    }

    Ok(())

}

pub async fn download(id: u64, src: Src, index: usize, nocache: bool, save_dir:&Option<String>, print: bool, climit: usize) -> Result<(), CommandError> {
    let requestor = RequestorBuilder::new()
        .ignore_cache(nocache)
        .build();

    match src {
        Src::ZBKYYY => dwonload_teleplay(create_teleplay(requestor, ZBKYYYParser::new(), id), index, save_dir, print, climit).await?,
        Src::IJUJITV => dwonload_teleplay(create_teleplay(requestor, IJUJITVParser::new(), id), index, save_dir, print, climit).await?,
        Src::JUGOUGOU => dwonload_teleplay(create_teleplay(requestor, JUGOUGOUParser::new(), id), index, save_dir, print, climit).await?,
    };
    Ok(())
}

pub async fn m3u8_download(url: &str, output: &str, climit: usize) -> Result<(), CommandError> {
    let path = std::path::Path::new(output);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).unwrap();
        }
    }
    let mut builder = M3U8DownloadBuilder::new();
    builder
        .uri(url)
        .save_file(output)
        .timeout(5)
        .climit(climit)
        .ignore_cache(true);
    let mut downloader = builder.build();
    downloader.download().await.unwrap();
    Ok(())
}