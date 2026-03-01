use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn write_test_wav(path: &Path) {
    let sample_rate = 44_100u32;
    let seconds = 0.3f32;
    let n = (sample_rate as f32 * seconds) as usize;

    let mut samples = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        let x = 0.55 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
            + 0.35 * (2.0 * std::f32::consts::PI * 1100.0 * t).sin()
            + 0.25 * (2.0 * std::f32::consts::PI * 4200.0 * t).sin();
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

    fs::write(path, bytes).expect("write wav");
}

fn unique_temp_dir(suffix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "moodbar-cli-png-test-{}-{}-{ts}",
        std::process::id(),
        suffix
    ));
    fs::create_dir_all(&dir).expect("create dir");
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
fn generate_png_output_is_valid() {
    let temp = unique_temp_dir("png");
    let input = temp.join("in.wav");
    let output = temp.join("out.png");
    write_test_wav(&input);

    let bin = moodbar_bin_path();
    let out = Command::new(&bin)
        .arg("--json")
        .arg("generate")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--format")
        .arg("png")
        .arg("--svg-shape")
        .arg("waveform")
        .output()
        .unwrap_or_else(|err| panic!("run png generate using {}: {err}", bin.display()));

    assert!(out.status.success());
    let png = fs::read(&output).expect("read png");
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"format\": \"png\""));
}
