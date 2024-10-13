use crate::args::Src;
use crate::downloader::{DownloadError, M3U8DownloadBuilder, MP4DownloadBuilder};
use crate::vrsr::error::Error as VRSRError;
use crate::vrsr::GeneralResource;
use crate::vrsr::IJUJITVParser;
use crate::vrsr::JUGOUGOUParser;
use crate::vrsr::ZBKYYYParser;
use crate::vrsr::{
    create_resource, create_teleplay, Episode, GeneralTeleplay, RequestorBuilder, Resource,
    Teleplay, URIType,
};
use crate::vrsr::{EpisodeParse, GenerateInfo, Request, ResourceParse, TeleplayParse};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use thiserror::Error;

async fn search_resource<'a, R, P>(
    mut resource: GeneralResource<'a, R, P>,
    arg_value: &str,
    keyword: &str,
) -> Result<(), VRSRError>
where
    R: Request,
    P: GenerateInfo + ResourceParse + TeleplayParse + EpisodeParse,
{
    println!("===========================");
    println!("{} [{}]", arg_value, resource.name().to_string());
    println!("===========================");
    let teleplays = resource.search(keyword).await?;
    for teleplay in teleplays.iter() {
        let teleplay_locked = teleplay.lock().await;
        println!("{}", teleplay_locked.info());
        println!("---------------------------");
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
    let requestor = RequestorBuilder::new().ignore_cache(nocache).build();
    if all {
        search_resource(
            create_resource(requestor.clone(), ZBKYYYParser::new()),
            "zbkyyy",
            keyword,
        )
        .await?;
        search_resource(
            create_resource(requestor.clone(), IJUJITVParser::new()),
            "ijujitv",
            keyword,
        )
        .await?;
        search_resource(
            create_resource(requestor.clone(), JUGOUGOUParser::new()),
            "jugougou",
            keyword,
        )
        .await?;
    } else {
        match src {
            Src::ZBKYYY => {
                search_resource(
                    create_resource(requestor.clone(), ZBKYYYParser::new()),
                    "zbkyyy",
                    keyword,
                )
                .await?
            }
            Src::IJUJITV => {
                search_resource(
                    create_resource(requestor.clone(), IJUJITVParser::new()),
                    "ijujitv",
                    keyword,
                )
                .await?
            }
            Src::JUGOUGOU => {
                search_resource(
                    create_resource(requestor.clone(), JUGOUGOUParser::new()),
                    "jugougou",
                    keyword,
                )
                .await?
            }
        }
    }
    Ok(())
}

async fn dwonload_teleplay<'a, R, P>(
    mut teleplay: GeneralTeleplay<R, P>,
    index: usize,
    save_dir: &Option<String>,
    print: bool,
    climit: usize,
) -> Result<(), VRSRError>
where
    R: Request,
    P: TeleplayParse + EpisodeParse,
{
    teleplay.request().await?;
    println!("{}", teleplay.info());
    let save_path = if let Some(save_dir) = save_dir {
        std::path::Path::new(save_dir)
    } else {
        std::path::Path::new(teleplay.title())
    };
    if !save_path.exists() {
        std::fs::create_dir_all(save_path)?;
    }
    let teleplay_src = teleplay.episodes();

    if print {
        for (index, result) in teleplay_src.iter().enumerate() {
            println!(
                "{} -> {}",
                index + 1,
                result.0.as_ref().unwrap_or(&"unknown".to_string())
            );
            for episode in result.1.iter() {
                let episode_locked = episode.lock().await;
                print!("[{}]", episode_locked.name());
            }
            println!();
        }
    } else {
        if let Some(result) = teleplay_src.get(index - 1) {
            let pbars = MultiProgress::new();
            let m3u8_style = ProgressStyle::with_template(
                "[{prefix}][{elapsed_precise}] {bar:100.cyan/blue} {pos:>4}/{len:4} {msg}",
            )
            .unwrap();
            let mp4_style = ProgressStyle::with_template(
                "[{prefix}][{elapsed_precise}] {bar:100.cyan/blue} {pos:>4}/{len:4}MB {msg}",
            )
            .unwrap();
            let default_style = ProgressStyle::with_template(
                "[{prefix}][{elapsed_precise}] {bar:100.cyan/blue} 已下载 {msg}",
            )
            .unwrap();
            let mut m3u8_builder = M3U8DownloadBuilder::new();
            let mut mp4_builder = MP4DownloadBuilder::new();
            let episode_count = result.1.len();
            for (index, episode) in result.1.iter().enumerate() {
                let mut episode_locked = episode.lock().await;
                let save_file_path = save_path.join(format!("{}.mp4", episode_locked.name()));
                let save_file = save_file_path.to_string_lossy().to_string();

                let pbar = pbars.add(ProgressBar::hidden());
                pbar.set_prefix(format!("{:02}/{:02}", index + 1, episode_count));
                pbar.set_message(save_file.clone());

                if save_file_path.exists() {
                    pbar.set_style(default_style.clone());
                    pbar.set_length(100);
                    pbar.set_position(100);
                    pbar.finish();
                    continue;
                }
                let uri = episode_locked.request().await?;

                match uri.utype {
                    URIType::M3U8 => {
                        pbar.set_style(m3u8_style.clone());
                        let mut downloader = m3u8_builder
                            .uri(uri.uri)
                            .pbar(pbar)
                            .timeout(3)
                            .climit(climit)
                            .save_file(&save_file)
                            .build();
                        downloader.download().await.unwrap();
                    }
                    URIType::MP4 => {
                        pbar.set_style(mp4_style.clone());
                        let mut downloader = mp4_builder
                            .uri(uri.uri)
                            .pbar(pbar)
                            .timeout(3)
                            .save_file(&save_file)
                            .build();
                        downloader.download().await.unwrap();
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

pub async fn download(
    id: u64,
    src: Src,
    index: usize,
    nocache: bool,
    save_dir: &Option<String>,
    print: bool,
    climit: usize,
) -> Result<(), CommandError> {
    let requestor = RequestorBuilder::new().ignore_cache(nocache).build();

    match src {
        Src::ZBKYYY => {
            dwonload_teleplay(
                create_teleplay(requestor, ZBKYYYParser::new(), id),
                index,
                save_dir,
                print,
                climit,
            )
            .await?
        }
        Src::IJUJITV => {
            dwonload_teleplay(
                create_teleplay(requestor, IJUJITVParser::new(), id),
                index,
                save_dir,
                print,
                climit,
            )
            .await?
        }
        Src::JUGOUGOU => {
            dwonload_teleplay(
                create_teleplay(requestor, JUGOUGOUParser::new(), id),
                index,
                save_dir,
                print,
                climit,
            )
            .await?
        }
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
