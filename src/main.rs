use std::path::PathBuf;

use clap::Parser;
use paper::Paper;

mod paper;
mod wgpu_layer;

#[derive(Parser)]
#[command(about, version)]
struct Cli {
    // Path to wgsl shader
    #[arg(value_name = "SHADER")]
    shader_path: PathBuf,
}

fn main() {
    let args = Cli::parse();
    let paper = Paper {
        shader: args.shader_path,
    };
    paper.run();
}
