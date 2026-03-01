use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn write_test_wav(path: &Path) {
    let sample_rate = 44_100u32;
    let seconds = 0.2f32;
    let n = (sample_rate as f32 * seconds) as usize;

    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        let x = 0.55 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
            + 0.35 * (2.0 * std::f32::consts::PI * 1100.0 * t).sin();
        samples.push(x.clamp(-1.0, 1.0));
    }

    let mut bytes = Vec::new();
    let data_bytes = (samples.len() * 2) as u32;

    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_bytes.to_le_bytes());
    for s in samples {
        let v = (s * i16::MAX as f32).round() as i16;
        bytes.extend_from_slice(&v.to_le_bytes());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create input dir");
    }
    fs::write(path, bytes).expect("write test wav");
}

fn unique_temp_dir(suffix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "moodbar-cli-batch-test-{}-{}-{ts}",
        std::process::id(),
        suffix
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn moodbar_bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_moodbar") {
        return PathBuf::from(p);
    }

    let mut p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/moodbar")
        .canonicalize()
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/moodbar")
        });
    if cfg!(windows) {
        p.set_extension("exe");
    }
    p
}

#[test]
fn batch_supports_parallel_jobs_flag() {
    let temp = unique_temp_dir("parallel");
    let input_dir = temp.join("in");
    let output_dir = temp.join("out");

    write_test_wav(&input_dir.join("a.wav"));
    write_test_wav(&input_dir.join("nested/b.wav"));
    write_test_wav(&input_dir.join("nested/c.wav"));
    write_test_wav(&input_dir.join("d.wav"));

    let bin = moodbar_bin_path();
    let out = Command::new(&bin)
        .arg("--json")
        .arg("batch")
        .arg("-i")
        .arg(&input_dir)
        .arg("-o")
        .arg(&output_dir)
        .arg("--jobs")
        .arg("2")
        .arg("--format")
        .arg("svg")
        .arg("--output-ext")
        .arg("svg")
        .output()
        .unwrap_or_else(|err| panic!("run batch using {}: {err}", bin.display()));

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"processed\": 4"));
    assert!(stdout.contains("\"succeeded\": 4"));
    assert!(stdout.contains("\"failed\": 0"));

    assert!(output_dir.join("a.svg").exists());
    assert!(output_dir.join("nested/b.svg").exists());
    assert!(output_dir.join("nested/c.svg").exists());
    assert!(output_dir.join("d.svg").exists());
}

#[test]
fn batch_skips_when_output_is_newer_or_equal() {
    let temp = unique_temp_dir("skip");
    let input_dir = temp.join("in");
    let output_dir = temp.join("out");

    write_test_wav(&input_dir.join("a.wav"));
    write_test_wav(&input_dir.join("nested/b.wav"));

    let bin = moodbar_bin_path();
    let first = Command::new(&bin)
        .arg("--json")
        .arg("batch")
        .arg("-i")
        .arg(&input_dir)
        .arg("-o")
        .arg(&output_dir)
        .arg("--jobs")
        .arg("2")
        .arg("--format")
        .arg("svg")
        .arg("--output-ext")
        .arg("svg")
        .output()
        .unwrap_or_else(|err| panic!("first batch run using {}: {err}", bin.display()));
    assert!(first.status.success());

    let second = Command::new(&bin)
        .arg("--json")
        .arg("batch")
        .arg("-i")
        .arg(&input_dir)
        .arg("-o")
        .arg(&output_dir)
        .arg("--jobs")
        .arg("2")
        .arg("--format")
        .arg("svg")
        .arg("--output-ext")
        .arg("svg")
        .output()
        .unwrap_or_else(|err| panic!("second batch run using {}: {err}", bin.display()));
    assert!(second.status.success());

    let stdout = String::from_utf8(second.stdout).expect("utf8 second stdout");
    assert!(stdout.contains("\"processed\": 2"));
    assert!(stdout.contains("\"succeeded\": 0"));
    assert!(stdout.contains("\"skipped\": 2"));
    assert!(stdout.contains("\"failed\": 0"));
}

#[test]
fn batch_progress_writes_status_lines() {
    let temp = unique_temp_dir("progress");
    let input_dir = temp.join("in");
    let output_dir = temp.join("out");

    write_test_wav(&input_dir.join("a.wav"));
    write_test_wav(&input_dir.join("b.wav"));

    let bin = moodbar_bin_path();
    let out = Command::new(&bin)
        .arg("--json")
        .arg("batch")
        .arg("-i")
        .arg(&input_dir)
        .arg("-o")
        .arg(&output_dir)
        .arg("--jobs")
        .arg("2")
        .arg("--progress")
        .arg("--format")
        .arg("svg")
        .arg("--output-ext")
        .arg("svg")
        .output()
        .unwrap_or_else(|err| panic!("run progress batch using {}: {err}", bin.display()));

    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).expect("utf8 stderr");
    assert!(stderr.contains("[1/2]") || stderr.contains("[2/2]"));
    assert!(stderr.contains("generated"));
}
