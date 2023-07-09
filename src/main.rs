use std::path::PathBuf;

use clap::Parser;
use paper::{Paper, PaperConfig};

mod paper;
mod wgpu_layer;

#[derive(Parser)]
#[command(about, version)]
struct Cli {
    // Name of the output (eg. HDMI-1, eDP-1)
    #[arg(short)]
    output_name: Option<String>,
    // Name of the output (eg. HDMI-1, eDP-1)
    #[arg(short = 'W')]
    width: Option<u32>,
    // Name of the output (eg. HDMI-1, eDP-1)
    #[arg(short = 'H')]
    height: Option<u32>,
    // Path to wgsl shader
    #[arg(value_name = "SHADER")]
    shader_path: PathBuf,
}

fn main() {
    let args = Cli::parse();
    if let Some(output_name) = &args.output_name {
        println!(
            "The shader will be loaded as soon as {} is registered.",
            output_name
        )
    } else {
        println!("The shader will be loaded on the first avaiable output.")
    }

    Paper::run(PaperConfig {
        output_name: args.output_name,
        width: args.width,
        height: args.height,
        shader_path: args.shader_path,
    });
}
