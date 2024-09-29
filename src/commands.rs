use crate::vrsr::error::Error as VRSRError;
use crate::m3u8::{DownloadError, M3U8DownloadBuilder};
use crate::vrsr::{create_resource, RequestorBuilder, Resource, Teleplay};
use crate::vrsr::request::Requestor;
use crate::vrsr::ZBKYYYParser;
use crate::vrsr::IJUJITVParser;
use thiserror::Error;
use crate::args::Src;
use std::path;
use std::sync::Arc;

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Parser resource error: {0:?}")]
    ParserResourceError(#[from] VRSRError),
    #[error("M3U8 download error: {0:?}")]
    M3U8DownloadError(#[from] DownloadError),
}

pub async fn search_zbkyyy(requestor: Arc<Requestor>, keyword: &str) -> Result<(), CommandError> {
    let parser = ZBKYYYParser::new();
    let mut resource = create_resource(requestor.clone(), parser);
    println!("@:zbkyyy[{}]", resource.name().to_string());
    let teleplays = resource.search(keyword).await?;
    for teleplay in teleplays.iter() {
        let teleplay_locked = teleplay.lock().await;
        println!("{}", teleplay_locked.info());
        println!("==============");
    }
    Ok(())
}

pub async fn search_iujitv(requestor: Arc<Requestor>, keyword: &str) -> Result<(), CommandError> {
    let parser = IJUJITVParser::new();
    let mut resource = create_resource(requestor.clone(), parser);
    println!("@:ijujitv[{}]", resource.name().to_string());
    let teleplays = resource.search(keyword).await?;
    for teleplay in teleplays.iter() {
        let teleplay_locked = teleplay.lock().await;
        println!("{}", teleplay_locked.info());
        println!("==============");
    }
    Ok(())
}

pub async fn search(keyword: &str, src: Src, all: bool, nocache: bool) -> Result<(), CommandError> {
    let requestor = RequestorBuilder::new()
        .ignore_cache(nocache)
        .build();
    if all {
        search_zbkyyy(requestor.clone(), keyword).await?;
        search_iujitv(requestor.clone(), keyword).await?;
    } else {
        match src {
            Src::ZBKYYY => search_zbkyyy(requestor, keyword).await?,
            Src::IJUJITV => search_iujitv(requestor, keyword).await?,
        }
    }
    Ok(())
}

pub async fn download(id: u64, src: Src, nocache: bool) -> Result<(), CommandError> {
    let requestor = RequestorBuilder::new()
        .ignore_cache(nocache)
        .build();
    Ok(())
}

pub async fn m3u8_download(url: &str, output: &str) -> Result<(), CommandError> {
    let path = path::Path::new(output);
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
        .ignore_cache(true);
    let mut downloader = builder.build();
    downloader.download().await.unwrap();
    Ok(())
}