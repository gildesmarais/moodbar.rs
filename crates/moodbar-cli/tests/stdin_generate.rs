use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(suffix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "moodbar-cli-stdin-test-{}-{}-{ts}",
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
fn generate_supports_stdin_f32le() {
    let temp = unique_temp_dir("f32");
    let output = temp.join("out.svg");

    let sample_rate = 44_100.0f32;
    let seconds = 0.25f32;
    let n = (sample_rate * seconds) as usize;
    let mut pcm = Vec::with_capacity(n * 4);
    for i in 0..n {
        let t = i as f32 / sample_rate;
        let x = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        pcm.extend_from_slice(&x.to_le_bytes());
    }

    let bin = moodbar_bin_path();
    let mut child = Command::new(&bin)
        .arg("--json")
        .arg("generate")
        .arg("--stdin")
        .arg("--stdin-format")
        .arg("f32-le")
        .arg("--sample-rate")
        .arg("44100")
        .arg("--channels")
        .arg("1")
        .arg("-o")
        .arg(&output)
        .arg("--format")
        .arg("svg")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("spawn {}: {err}", bin.display()));

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(&pcm)
        .expect("write stdin");

    let out = child.wait_with_output().expect("wait child");
    assert!(out.status.success());

    let svg = fs::read_to_string(&output).expect("read output svg");
    assert!(svg.contains("<svg"));
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"input\": \"<stdin>\""));
}
