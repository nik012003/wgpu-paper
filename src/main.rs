use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use paper::{Paper, PaperConfig};
use smithay_client_toolkit::shell::wlr_layer::Anchor;
use strum::Display;
mod paper;
mod wgpu_layer;

#[derive(Parser)]
#[command(about, version)]
struct Cli {
    /// Name of the output (eg. HDMI-1, eDP-1)
    #[arg(short)]
    output_name: Option<String>,
    /// Width, deafults to the screen width
    #[arg(short = 'W')]
    width: Option<u32>,
    /// Height, deafults to the screen height
    #[arg(short = 'H')]
    height: Option<u32>,
    /// Comma sperated list of corners to anchor to
    #[arg(short = 'A', value_delimiter = ',', default_values_t = [ArgAnchor::Bottom])]
    anchor: Vec<ArgAnchor>,
    /// Path to wgsl shader
    #[arg(value_name = "SHADER")]
    shader_path: PathBuf,
}
#[derive(ValueEnum, Display, Clone)]
#[strum(serialize_all = "lowercase")]
enum ArgAnchor {
    Top,
    Bottom,
    Left,
    Right,
}

impl From<ArgAnchor> for Anchor {
    fn from(other: ArgAnchor) -> Anchor {
        match other {
            ArgAnchor::Top => Anchor::TOP,
            ArgAnchor::Bottom => Anchor::BOTTOM,
            ArgAnchor::Left => Anchor::LEFT,
            ArgAnchor::Right => Anchor::RIGHT,
        }
    }
}

fn main() {
    let mut args = Cli::parse();

    if let Some(output_name) = &args.output_name {
        println!(
            "The shader will be loaded as soon as {} is registered.",
            output_name
        )
    } else {
        println!("The shader will be loaded on the first avaiable output.")
    }

    let mut anchor: Anchor = args.anchor.remove(0).into();

    for ele in args.anchor {
        anchor |= ele.into();
    }

    Paper::run(PaperConfig {
        output_name: args.output_name,
        width: args.width,
        height: args.height,
        anchor,
        shader_path: args.shader_path,
    });
}
