use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use moodbar_core::{generate_moodbar_from_path, GenerateOptions, NormalizeMode};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Fixture {
    name: String,
    input_audio: String,
    expected_mood: String,
    #[serde(default)]
    options: FixtureOptions,
    #[serde(default = "default_max_mean_abs_diff")]
    max_mean_abs_diff: f64,
    #[serde(default = "default_max_abs_diff")]
    max_abs_diff: u8,
}

#[derive(Debug, Deserialize)]
struct FixtureOptions {
    #[serde(default = "default_fft_size")]
    fft_size: usize,
    #[serde(default = "default_low_cut")]
    low_cut_hz: f32,
    #[serde(default = "default_mid_cut")]
    mid_cut_hz: f32,
    #[serde(default)]
    normalize_mode: FixtureNormalizeMode,
    #[serde(default = "default_floor")]
    deterministic_floor: f64,
}

impl Default for FixtureOptions {
    fn default() -> Self {
        Self {
            fft_size: default_fft_size(),
            low_cut_hz: default_low_cut(),
            mid_cut_hz: default_mid_cut(),
            normalize_mode: FixtureNormalizeMode::default(),
            deterministic_floor: default_floor(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum FixtureNormalizeMode {
    #[default]
    PerChannelPeak,
    GlobalPeak,
}

impl FixtureNormalizeMode {
    fn to_core(&self) -> NormalizeMode {
        match self {
            FixtureNormalizeMode::PerChannelPeak => NormalizeMode::PerChannelPeak,
            FixtureNormalizeMode::GlobalPeak => NormalizeMode::GlobalPeak,
        }
    }
}

fn default_fft_size() -> usize {
    2048
}

fn default_low_cut() -> f32 {
    500.0
}

fn default_mid_cut() -> f32 {
    2000.0
}

fn default_floor() -> f64 {
    1e-12
}

fn default_max_mean_abs_diff() -> f64 {
    5.0
}

fn default_max_abs_diff() -> u8 {
    32
}

#[test]
fn parity_against_legacy_fixtures() -> Result<()> {
    let fixture_dir = fixture_root();
    let mut manifest_paths = collect_fixture_manifests(&fixture_dir)?;
    manifest_paths.sort();

    if manifest_paths.is_empty() {
        eprintln!(
            "no legacy fixtures found in {}; skipping parity test",
            fixture_dir.display()
        );
        return Ok(());
    }

    for manifest in manifest_paths {
        run_fixture(&manifest)?;
    }

    Ok(())
}

fn run_fixture(manifest_path: &Path) -> Result<()> {
    let content = fs::read_to_string(manifest_path).with_context(|| {
        format!(
            "failed to read fixture manifest {}",
            manifest_path.display()
        )
    })?;
    let fixture: Fixture = serde_json::from_str(&content)
        .with_context(|| format!("invalid fixture manifest {}", manifest_path.display()))?;

    let root = manifest_path
        .parent()
        .context("fixture manifest has no parent directory")?;
    let input_audio = root.join(&fixture.input_audio);
    let expected_mood = root.join(&fixture.expected_mood);

    let options = GenerateOptions {
        fft_size: fixture.options.fft_size,
        low_cut_hz: fixture.options.low_cut_hz,
        mid_cut_hz: fixture.options.mid_cut_hz,
        normalize_mode: fixture.options.normalize_mode.to_core(),
        deterministic_floor: fixture.options.deterministic_floor,
    };

    let actual = generate_moodbar_from_path(&input_audio, &options)
        .with_context(|| format!("generation failed for fixture {}", fixture.name))?;
    let expected = fs::read(&expected_mood)
        .with_context(|| format!("missing expected mood file for fixture {}", fixture.name))?;

    anyhow::ensure!(
        actual.len() == expected.len(),
        "fixture {} length mismatch: actual {} vs expected {}",
        fixture.name,
        actual.len(),
        expected.len()
    );

    let mut sum_diff = 0u64;
    let mut max_diff = 0u8;
    for (a, e) in actual.iter().zip(expected.iter()) {
        let diff = a.abs_diff(*e);
        sum_diff += diff as u64;
        max_diff = max_diff.max(diff);
    }

    let mean_abs_diff = sum_diff as f64 / actual.len() as f64;
    anyhow::ensure!(
        mean_abs_diff <= fixture.max_mean_abs_diff,
        "fixture {} mean abs diff too high: {:.3} > {:.3}",
        fixture.name,
        mean_abs_diff,
        fixture.max_mean_abs_diff
    );
    anyhow::ensure!(
        max_diff <= fixture.max_abs_diff,
        "fixture {} max abs diff too high: {} > {}",
        fixture.name,
        max_diff,
        fixture.max_abs_diff
    );

    Ok(())
}

fn collect_fixture_manifests(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut manifests = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) == Some("json") {
            manifests.push(path);
        }
    }
    Ok(manifests)
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/legacy")
        .canonicalize()
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/legacy")
        })
}
