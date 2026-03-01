use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use moodbar_core::{
    analysis_to_raw_rgb_bytes, analyze_path, render_svg, DetectionMode, GenerateOptions,
    NormalizeMode, SvgOptions, SvgShape,
};
use rayon::prelude::*;
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
        #[command(flatten)]
        dsp: DspArgs,
        #[command(flatten)]
        render: RenderArgs,
        #[arg(long)]
        force: bool,
    },

    /// Recursively generate moodbar files from a directory.
    Batch {
        #[arg(short = 'i', long)]
        input_dir: PathBuf,
        #[arg(short = 'o', long)]
        output_dir: PathBuf,
        #[command(flatten)]
        dsp: DspArgs,
        #[command(flatten)]
        render: RenderArgs,
        #[arg(long, default_value = "mood")]
        output_ext: String,
        #[arg(long, default_value_t = 0, help = "Parallel worker count (0 = auto)")]
        jobs: usize,
        #[arg(long)]
        force: bool,
    },

    /// Print basic information about an existing moodbar file.
    Inspect {
        #[arg(short = 'i', long)]
        input: PathBuf,
    },
}

#[derive(Args, Debug, Clone)]
struct DspArgs {
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
}

#[derive(Args, Debug, Clone)]
struct RenderArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::RawRgbV1)]
    format: OutputFormat,
    #[arg(long, value_enum, default_value_t = SvgShapeArg::Strip)]
    svg_shape: SvgShapeArg,
    #[arg(long, default_value_t = 1200)]
    svg_width: u32,
    #[arg(long, default_value_t = 96)]
    svg_height: u32,
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
    skipped: usize,
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
            dsp,
            render,
            force,
        } => {
            ensure_can_write(&output, force)?;

            let options = build_options(&dsp);
            let analysis = analyze_path(&input, &options)
                .with_context(|| format!("failed to generate moodbar for {}", input.display()))?;

            let bytes_written = match render.format {
                OutputFormat::RawRgbV1 => {
                    let bytes = analysis_to_raw_rgb_bytes(&analysis);
                    write_bytes(&output, &bytes)?;
                    bytes.len()
                }
                OutputFormat::Svg => {
                    let svg = render_svg(
                        &analysis,
                        &SvgOptions {
                            width: render.svg_width,
                            height: render.svg_height,
                            shape: render.svg_shape.into_core(),
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
                format: match render.format {
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
            dsp,
            render,
            output_ext,
            jobs,
            force,
        } => {
            let options = build_options(&dsp);
            let candidates = WalkDir::new(&input_dir)
                .follow_links(false)
                .into_iter()
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_type().is_file())
                .filter(|e| looks_like_audio(e.path()))
                .map(|e| e.path().to_path_buf())
                .collect::<Vec<_>>();

            let processed = candidates.len();
            let input_dir = Arc::new(input_dir);
            let output_dir = Arc::new(output_dir);
            let output_ext = Arc::new(output_ext);
            let options = Arc::new(options);
            let render = Arc::new(render);

            let worker_count = if jobs == 0 {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1)
            } else {
                jobs.max(1)
            };

            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(worker_count)
                .build()
                .context("failed to build rayon thread pool")?;

            let outcomes = pool.install(|| {
                candidates
                    .par_iter()
                    .map(|src| {
                        let rel = src.strip_prefix(&*input_dir).unwrap_or(src);
                        let mut dst = output_dir.join(rel);
                        dst.set_extension(output_ext.as_ref());
                        process_batch_item(src, &dst, &options, &render, force)
                            .with_context(|| format!("{} -> {}", src.display(), dst.display()))
                    })
                    .collect::<Vec<_>>()
            });

            let mut succeeded = 0usize;
            let skipped = 0usize;
            let mut failed = 0usize;
            for (src, outcome) in candidates.iter().zip(outcomes.into_iter()) {
                match outcome {
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
                            eprintln!("failed: {} ({err:#})", src.display());
                        }
                    }
                }
            }

            let result = BatchResult {
                processed,
                succeeded,
                skipped,
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

fn build_options(dsp: &DspArgs) -> GenerateOptions {
    GenerateOptions {
        fft_size: dsp.fft_size,
        low_cut_hz: dsp.low_cut_hz,
        mid_cut_hz: dsp.mid_cut_hz,
        normalize_mode: dsp.normalize_mode.into_core(),
        deterministic_floor: dsp.deterministic_floor,
        detection_mode: dsp.detection_mode.into_core(),
        frames_per_color: dsp.frames_per_color,
        band_edges_hz: dsp.band_edges_hz.clone(),
    }
}

fn process_batch_item(
    src: &Path,
    dst: &Path,
    options: &GenerateOptions,
    render: &RenderArgs,
    force: bool,
) -> Result<()> {
    ensure_can_write(dst, force)?;
    let analysis = analyze_path(src, options)
        .with_context(|| format!("decode/generate failed for {}", src.display()))?;

    match render.format {
        OutputFormat::RawRgbV1 => {
            let bytes = analysis_to_raw_rgb_bytes(&analysis);
            write_bytes(dst, &bytes)?;
        }
        OutputFormat::Svg => {
            let svg = render_svg(
                &analysis,
                &SvgOptions {
                    width: render.svg_width,
                    height: render.svg_height,
                    shape: render.svg_shape.into_core(),
                    ..SvgOptions::default()
                },
            );
            write_text(dst, &svg)?;
        }
    }
    Ok(())
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
