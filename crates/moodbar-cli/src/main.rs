use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use moodbar_core::{
    analysis_to_raw_rgb_bytes, analyze_path, render_svg, DetectionMode, GenerateOptions,
    NormalizeMode, SvgOptions, SvgShape,
};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "moodbar")]
#[command(about = "Generate moodbar data from audio files", long_about = None)]
struct Cli {
    #[arg(long, global = true, help = "Emit machine-readable JSON output")]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate a single moodbar file.
    Generate {
        #[arg(short = 'i', long)]
        input: PathBuf,
        #[arg(short = 'o', long)]
        output: PathBuf,
        #[arg(long, default_value_t = 2048)]
        fft_size: usize,
        #[arg(long, default_value_t = 500.0)]
        low_cut_hz: f32,
        #[arg(long, default_value_t = 2000.0)]
        mid_cut_hz: f32,
        #[arg(long, value_enum, default_value_t = NormalizeModeArg::PerChannelPeak)]
        normalize_mode: NormalizeModeArg,
        #[arg(long, default_value_t = 1e-12)]
        deterministic_floor: f64,
        #[arg(long, value_enum, default_value_t = DetectionModeArg::SpectralEnergy)]
        detection_mode: DetectionModeArg,
        #[arg(long, default_value_t = 1)]
        frames_per_color: usize,
        #[arg(long, value_delimiter = ',')]
        band_edges_hz: Vec<f32>,
        #[arg(long)]
        force: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::RawRgbV1)]
        format: OutputFormat,
        #[arg(long, value_enum, default_value_t = SvgShapeArg::Strip)]
        svg_shape: SvgShapeArg,
        #[arg(long, default_value_t = 1200)]
        svg_width: u32,
        #[arg(long, default_value_t = 96)]
        svg_height: u32,
    },

    /// Recursively generate moodbar files from a directory.
    Batch {
        #[arg(short = 'i', long)]
        input_dir: PathBuf,
        #[arg(short = 'o', long)]
        output_dir: PathBuf,
        #[arg(long, default_value_t = 2048)]
        fft_size: usize,
        #[arg(long, default_value_t = 500.0)]
        low_cut_hz: f32,
        #[arg(long, default_value_t = 2000.0)]
        mid_cut_hz: f32,
        #[arg(long, value_enum, default_value_t = NormalizeModeArg::PerChannelPeak)]
        normalize_mode: NormalizeModeArg,
        #[arg(long, default_value_t = 1e-12)]
        deterministic_floor: f64,
        #[arg(long, value_enum, default_value_t = DetectionModeArg::SpectralEnergy)]
        detection_mode: DetectionModeArg,
        #[arg(long, default_value_t = 1)]
        frames_per_color: usize,
        #[arg(long, value_delimiter = ',')]
        band_edges_hz: Vec<f32>,
        #[arg(long, default_value = "mood")]
        output_ext: String,
        #[arg(long)]
        force: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::RawRgbV1)]
        format: OutputFormat,
        #[arg(long, value_enum, default_value_t = SvgShapeArg::Strip)]
        svg_shape: SvgShapeArg,
        #[arg(long, default_value_t = 1200)]
        svg_width: u32,
        #[arg(long, default_value_t = 96)]
        svg_height: u32,
    },

    /// Print basic information about an existing moodbar file.
    Inspect {
        #[arg(short = 'i', long)]
        input: PathBuf,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    RawRgbV1,
    Svg,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum NormalizeModeArg {
    PerChannelPeak,
    GlobalPeak,
}

impl NormalizeModeArg {
    fn into_core(self) -> NormalizeMode {
        match self {
            NormalizeModeArg::PerChannelPeak => NormalizeMode::PerChannelPeak,
            NormalizeModeArg::GlobalPeak => NormalizeMode::GlobalPeak,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum DetectionModeArg {
    SpectralEnergy,
    SpectralFlux,
}

impl DetectionModeArg {
    fn into_core(self) -> DetectionMode {
        match self {
            DetectionModeArg::SpectralEnergy => DetectionMode::SpectralEnergy,
            DetectionModeArg::SpectralFlux => DetectionMode::SpectralFlux,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum SvgShapeArg {
    Strip,
    Waveform,
}

impl SvgShapeArg {
    fn into_core(self) -> SvgShape {
        match self {
            SvgShapeArg::Strip => SvgShape::Strip,
            SvgShapeArg::Waveform => SvgShape::Waveform,
        }
    }
}

#[derive(Serialize)]
struct GenerateResult {
    input: String,
    output: String,
    bytes_written: usize,
    frames: usize,
    channels: usize,
    format: String,
}

#[derive(Serialize)]
struct BatchResult {
    processed: usize,
    succeeded: usize,
    failed: usize,
}

#[derive(Serialize)]
struct InspectResult {
    input: String,
    bytes: usize,
    frames: usize,
}

fn main() {
    let cli = Cli::parse();
    let exit_code = match run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            2
        }
    };
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> Result<i32> {
    match cli.command {
        Command::Generate {
            input,
            output,
            fft_size,
            low_cut_hz,
            mid_cut_hz,
            normalize_mode,
            deterministic_floor,
            detection_mode,
            frames_per_color,
            band_edges_hz,
            force,
            format,
            svg_shape,
            svg_width,
            svg_height,
        } => {
            ensure_can_write(&output, force)?;

            let options = build_options(
                fft_size,
                low_cut_hz,
                mid_cut_hz,
                normalize_mode,
                deterministic_floor,
                detection_mode,
                frames_per_color,
                band_edges_hz,
            );
            let analysis = analyze_path(&input, &options)
                .with_context(|| format!("failed to generate moodbar for {}", input.display()))?;

            let bytes_written = match format {
                OutputFormat::RawRgbV1 => {
                    let bytes = analysis_to_raw_rgb_bytes(&analysis);
                    write_bytes(&output, &bytes)?;
                    bytes.len()
                }
                OutputFormat::Svg => {
                    let svg = render_svg(
                        &analysis,
                        &SvgOptions {
                            width: svg_width,
                            height: svg_height,
                            shape: svg_shape.into_core(),
                            ..SvgOptions::default()
                        },
                    );
                    write_text(&output, &svg)?;
                    svg.len()
                }
            };

            let result = GenerateResult {
                input: input.display().to_string(),
                output: output.display().to_string(),
                bytes_written,
                frames: analysis.frames.len(),
                channels: analysis.channel_count,
                format: match format {
                    OutputFormat::RawRgbV1 => "raw_rgb_v1",
                    OutputFormat::Svg => "svg",
                }
                .to_string(),
            };
            print_result(cli.json, &result)?;
            Ok(0)
        }
        Command::Batch {
            input_dir,
            output_dir,
            fft_size,
            low_cut_hz,
            mid_cut_hz,
            normalize_mode,
            deterministic_floor,
            detection_mode,
            frames_per_color,
            band_edges_hz,
            output_ext,
            force,
            format,
            svg_shape,
            svg_width,
            svg_height,
        } => {
            let options = build_options(
                fft_size,
                low_cut_hz,
                mid_cut_hz,
                normalize_mode,
                deterministic_floor,
                detection_mode,
                frames_per_color,
                band_edges_hz,
            );

            let mut processed = 0usize;
            let mut succeeded = 0usize;
            let mut failed = 0usize;

            for entry in WalkDir::new(&input_dir)
                .follow_links(false)
                .into_iter()
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_type().is_file())
            {
                let src = entry.path();
                if !looks_like_audio(src) {
                    continue;
                }

                processed += 1;
                let rel = src.strip_prefix(&input_dir).unwrap_or(src);
                let mut dst = output_dir.join(rel);
                dst.set_extension(&output_ext);

                let op = || -> Result<()> {
                    ensure_can_write(&dst, force)?;
                    let analysis = analyze_path(src, &options)
                        .with_context(|| format!("decode/generate failed for {}", src.display()))?;

                    match format {
                        OutputFormat::RawRgbV1 => {
                            let bytes = analysis_to_raw_rgb_bytes(&analysis);
                            write_bytes(&dst, &bytes)?;
                        }
                        OutputFormat::Svg => {
                            let svg = render_svg(
                                &analysis,
                                &SvgOptions {
                                    width: svg_width,
                                    height: svg_height,
                                    shape: svg_shape.into_core(),
                                    ..SvgOptions::default()
                                },
                            );
                            write_text(&dst, &svg)?;
                        }
                    }
                    Ok(())
                };

                match op() {
                    Ok(()) => succeeded += 1,
                    Err(err) => {
                        failed += 1;
                        if cli.json {
                            eprintln!(
                                "{{\"input\":\"{}\",\"error\":\"{}\"}}",
                                src.display(),
                                escape_json_string(&format!("{err:#}"))
                            );
                        } else {
                            eprintln!("failed: {} -> {} ({err:#})", src.display(), dst.display());
                        }
                    }
                }
            }

            let result = BatchResult {
                processed,
                succeeded,
                failed,
            };
            print_result(cli.json, &result)?;
            Ok(if failed == 0 { 0 } else { 1 })
        }
        Command::Inspect { input } => {
            let bytes = fs::read(&input)?;
            let result = InspectResult {
                input: input.display().to_string(),
                bytes: bytes.len(),
                frames: bytes.len() / 3,
            };
            print_result(cli.json, &result)?;
            Ok(0)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_options(
    fft_size: usize,
    low_cut_hz: f32,
    mid_cut_hz: f32,
    normalize_mode: NormalizeModeArg,
    deterministic_floor: f64,
    detection_mode: DetectionModeArg,
    frames_per_color: usize,
    band_edges_hz: Vec<f32>,
) -> GenerateOptions {
    GenerateOptions {
        fft_size,
        low_cut_hz,
        mid_cut_hz,
        normalize_mode: normalize_mode.into_core(),
        deterministic_floor,
        detection_mode: detection_mode.into_core(),
        frames_per_color,
        band_edges_hz,
    }
}

fn ensure_can_write(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        anyhow::bail!("{} exists; pass --force to overwrite", path.display());
    }
    Ok(())
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text.as_bytes())?;
    Ok(())
}

fn print_result<T: Serialize>(json: bool, value: &T) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        match serde_json::to_value(value)? {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    println!("{k}: {v}");
                }
            }
            other => println!("{other}"),
        }
    }
    Ok(())
}

fn looks_like_audio(path: &Path) -> bool {
    let ext = path.extension().and_then(OsStr::to_str).unwrap_or_default();
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mp3" | "ogg" | "flac" | "wav" | "m4a" | "aac" | "opus"
    )
}

fn escape_json_string(s: &str) -> String {
    serde_json::to_string(s)
        .unwrap_or_else(|_| "\"<encode-error>\"".to_string())
        .trim_matches('"')
        .to_string()
}
