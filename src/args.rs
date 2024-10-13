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
        #[arg(short, long, default_value = "jugougou")]
        src: Src,
        #[arg(short, long)]
        all: bool,
        #[arg(long)]
        nocache: bool,
    },
    /// Download a video from a platform
    Download {
        id: u64,
        #[arg(short, long, default_value = "jugougou")]
        src: Src,
        #[arg(short, long, default_value = "1")]
        index: usize,
        #[arg(long)]
        nocache: bool,
        #[arg(long)]
        save_dir: Option<String>,
        #[arg(short, long)]
        print: bool,
        #[arg(short, long, default_value = "32")]
        climit: usize,
    },
    /// Convert a video to M3U8 format
    M3U8 {
        url: String,
        #[arg(short, long, default_value = "output.mp4")]
        output: String,
        #[arg(short, long, value_parser = clap::value_parser!(u64).range(1..), default_value = "32")]
        climit: usize,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Src {
    ZBKYYY,
    IJUJITV,
    JUGOUGOU,
}
