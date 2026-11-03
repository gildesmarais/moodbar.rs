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

fn run_stdin_generate(args: &[&str], output: &Path, pcm: &[u8]) -> std::process::Output {
    let bin = moodbar_bin_path();
    let mut child = Command::new(&bin)
        .args(["--json", "generate", "--stdin", "--force", "-o"])
        .arg(output)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("spawn {}: {err}", bin.display()));
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(pcm)
        .expect("write stdin");
    child.wait_with_output().expect("wait child")
}

fn sine_f32le(seconds: f32) -> Vec<u8> {
    let sample_rate = 44_100.0f32;
    let n = (sample_rate * seconds) as usize;
    (0..n)
        .flat_map(|i| {
            let t = i as f32 / sample_rate;
            ((2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5).to_le_bytes()
        })
        .collect()
}

#[test]
fn generate_supports_stdin_f32le() {
    let temp = unique_temp_dir("f32");
    let output = temp.join("out.svg");
    let args = [
        "--stdin-format",
        "f32-le",
        "--sample-rate",
        "44100",
        "--channels",
        "1",
        "--format",
        "svg",
    ];
    let out = run_stdin_generate(&args, &output, &sine_f32le(0.25));
    assert!(out.status.success());

    let svg = fs::read_to_string(&output).expect("read output svg");
    assert!(svg.contains("<svg"));
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"input\": \"<stdin>\""));
}

#[test]
fn generate_stdin_rejects_invalid_options_without_panicking() {
    let temp = unique_temp_dir("invalid");
    let output = temp.join("out.bin");
    let cases: [(&[&str], &str); 5] = [
        (&["--fft-size", "0"], "fft_size"),
        (&["--band-edges-hz", "NaN"], "finite"),
        (&["--sample-rate", "0"], "sample_rate"),
        (&["--colors", "€€"], "invalid color"),
        (
            &[
                "--format",
                "png",
                "--svg-width",
                "65536",
                "--svg-height",
                "16384",
            ],
            "png dimensions",
        ),
    ];
    for (args, expected) in cases {
        let out = run_stdin_generate(args, &output, &sine_f32le(0.1));
        let stderr = String::from_utf8(out.stderr).expect("utf8 stderr");
        assert_eq!(out.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains(expected), "{args:?}: {stderr}");
        assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");
    }
}
