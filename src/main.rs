use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use paper::{Margin, Paper, PaperConfig};
use smithay_client_toolkit::shell::wlr_layer::Anchor;
use strum::Display;
mod paper;
mod wgpu_layer;

#[derive(Parser)]
#[command(about, version)]
struct Cli {
    /// Name of the output (eg. HDMI-1, eDP-1)
    #[arg(long, short)]
    output_name: Option<String>,
    /// Width, deafults to the screen width
    #[arg(long, short = 'W')]
    width: Option<u32>,
    /// Height, deafults to the screen height
    #[arg(long, short = 'H')]
    height: Option<u32>,
    /// Margin from the top
    #[arg(long, short = 'T', default_value_t = 0)]
    margin_top: i32,
    /// Margin from the top
    #[arg(long, short = 'R', default_value_t = 0)]
    margin_right: i32,
    /// Margin from the top
    #[arg(long, short = 'B', default_value_t = 0)]
    margin_bottom: i32,
    /// Margin from the top
    #[arg(long, short = 'L', default_value_t = 0)]
    margin_left: i32,
    /// Comma sperated list of corners to anchor to
    #[arg(long, short = 'A', value_delimiter = ',', default_values_t = [ArgAnchor::Bottom])]
    anchor: Vec<ArgAnchor>,
    /// Number of pointer positions given to shader
    #[arg(long, short, default_value_t = 10)]
    pointer_trail_frames: usize,
    /// Frames per second, higher values than vsync won't work
    #[arg(long, short)]
    fps: Option<u64>,
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
        margin: Margin {
            top: args.margin_top,
            right: args.margin_right,
            bottom: args.margin_bottom,
            left: args.margin_left,
        },
        pointer_trail_frames: args.pointer_trail_frames,
        fps: args.fps,
        shader_path: args.shader_path,
    });
}
