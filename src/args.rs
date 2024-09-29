use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(version, author, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub mode: Option<Mode>,
}

#[derive(Subcommand)]
pub enum Mode {
    /// Search for videos on various platforms
    Search {
        keyword: String,
        #[arg(short, long, default_value = "zbkyyy")]
        src: Src,
        #[arg(short, long)]
        all: bool,
        #[arg(long)]
        nocache: bool,
    },
    /// Download a video from a platform
    Download {
        id: u64,
        #[arg(short, long, default_value = "zbkyyy")]
        src: Src,
        #[arg(long)]
        nocache: bool,
    },
    /// Convert a video to M3U8 format
    M3U8 {
        url: String,
        #[arg(short, long, default_value = "output.mp4")]
        output: String,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Src {
    ZBKYYY,
    IJUJITV,
}