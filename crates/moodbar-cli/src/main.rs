use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use moodbar_core::{generate_moodbar_from_path, GenerateOptions, NormalizeMode};
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
        #[arg(long)]
        force: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::RawRgbV1)]
        format: OutputFormat,
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
        #[arg(long, default_value = "mood")]
        output_ext: String,
        #[arg(long)]
        force: bool,
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

#[derive(Serialize)]
struct GenerateResult {
    input: String,
    output: String,
    bytes_written: usize,
    frames: usize,
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
            force,
            format,
        } => {
            let _ = format;
            ensure_can_write(&output, force)?;

            let options = GenerateOptions {
                fft_size,
                low_cut_hz,
                mid_cut_hz,
                normalize_mode: normalize_mode.into_core(),
                deterministic_floor,
            };
            let bytes = generate_moodbar_from_path(&input, &options)
                .with_context(|| format!("failed to generate moodbar for {}", input.display()))?;

            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output, &bytes)?;

            let result = GenerateResult {
                input: input.display().to_string(),
                output: output.display().to_string(),
                bytes_written: bytes.len(),
                frames: bytes.len() / 3,
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
            output_ext,
            force,
        } => {
            let options = GenerateOptions {
                fft_size,
                low_cut_hz,
                mid_cut_hz,
                normalize_mode: normalize_mode.into_core(),
                deterministic_floor,
            };

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
                    let bytes = generate_moodbar_from_path(src, &options)
                        .with_context(|| format!("decode/generate failed for {}", src.display()))?;
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&dst, &bytes)?;
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

fn ensure_can_write(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        anyhow::bail!("{} exists; pass --force to overwrite", path.display());
    }
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
