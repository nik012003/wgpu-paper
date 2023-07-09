use std::path::PathBuf;

use clap::Parser;
use paper::Paper;

mod paper;
mod wgpu_layer;

#[derive(Parser)]
#[command(about, version)]
struct Cli {
    // Name of the output (eg. HDMI-1, eDP-1)
    #[arg(short)]
    output_name: Option<String>,
    // Path to wgsl shader
    #[arg(value_name = "SHADER")]
    shader_path: PathBuf,
}

fn main() {
    let args = Cli::parse();
    Paper::run(args.shader_path, args.output_name);
}
