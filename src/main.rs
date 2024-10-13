mod args;
mod commands;
mod downloader;
mod vrsr;

use args::{Cli, Mode};
use clap::Parser;
use commands::{download, m3u8_download, search, CommandError};
use downloader::{DownloadError, M3U8DownloadBuilder};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), CommandError> {
    env_logger::init();
    let cli = Cli::parse();
    if let Some(mode) = cli.mode {
        match mode {
            Mode::Search {
                keyword,
                src,
                all,
                nocache,
            } => {
                search(&keyword, src, all, nocache).await?;
            }
            Mode::Download {
                id,
                src,
                index,
                nocache,
                save_dir,
                print,
                climit,
            } => {
                download(id, src, index, nocache, &save_dir, print, climit).await?;
            }
            Mode::M3U8 {
                url,
                output,
                climit,
            } => {
                m3u8_download(&url, &output, climit).await?;
            }
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
