mod m3u8;
mod source;
use source::{FilmInfo, ZBKYYY};
use m3u8::{DownloadError, M3U8DownloadBuilder};

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
            .timeout(10)
            .save_file(save_file)
            .pbar(pbar)
            .build();
        downloader.download().await?;

        cmd.spawn().unwrap();
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), DownloadError> {
    env_logger::init();
    let url = "https://www.zbkyyy.com/qyvoddetail/8391.html";
    let mut zbkyyy = ZBKYYY::new(url);
    zbkyyy.parse().await?;
    download(&zbkyyy).await?;
    Ok(())
}
