//! `--quiet` must have a real, observable effect: status/banner stdout from
//! the transcode path is suppressed, while command results (probe JSON) and
//! stderr warnings keep printing.

mod common;

use assert_cmd::Command;

/// Without `--quiet`, a WAV→FLAC transcode prints its plan and summary.
#[test]
fn transcode_without_quiet_prints_status() {
    let (dir, wav) = common::write_wav_fixture(440.0, 48_000, 1, 1.0);
    let out = dir.path().join("out_loud.flac");

    let assert = Command::cargo_bin("oximedia")
        .expect("binary builds")
        .args([
            "transcode",
            "-i",
            wav.to_str().expect("utf8 path"),
            "-o",
            out.to_str().expect("utf8 path"),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        stdout.contains("Transcode Plan"),
        "default run must print the plan, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Transcode Complete"),
        "default run must print the summary, got:\n{stdout}"
    );
    assert!(out.exists(), "output file must be written");
}

/// With `--quiet`, the same transcode emits NOTHING on stdout — but still
/// performs the real work (the output file exists).
#[test]
fn transcode_with_quiet_suppresses_status() {
    let (dir, wav) = common::write_wav_fixture(440.0, 48_000, 1, 1.0);
    let out = dir.path().join("out_quiet.flac");

    let assert = Command::cargo_bin("oximedia")
        .expect("binary builds")
        .args([
            "--quiet",
            "transcode",
            "-i",
            wav.to_str().expect("utf8 path"),
            "-o",
            out.to_str().expect("utf8 path"),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        stdout.trim().is_empty(),
        "--quiet must suppress all transcode status stdout, got:\n{stdout}"
    );
    assert!(
        out.exists(),
        "output file must still be written under --quiet"
    );
}

/// `--quiet` must NOT suppress command results: probe --format json still
/// prints its JSON payload (machine consumers rely on this).
#[test]
fn quiet_does_not_suppress_probe_json() {
    let (_dir, wav) = common::write_wav_fixture(440.0, 48_000, 1, 1.0);

    let assert = Command::cargo_bin("oximedia")
        .expect("binary builds")
        .args([
            "--quiet",
            "probe",
            "-i",
            wav.to_str().expect("utf8 path"),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("probe --format json must emit valid JSON");
    assert!(
        parsed.is_object(),
        "probe JSON payload must survive --quiet"
    );
}

/// Warn-and-proceed diagnostics stay visible under `--quiet`: they go to
/// stderr, which only errors and warnings may use.
#[test]
fn quiet_keeps_stderr_warnings() {
    let (dir, wav) = common::write_wav_fixture(440.0, 48_000, 1, 1.0);
    let out = dir.path().join("out_warn.flac");

    let assert = Command::cargo_bin("oximedia")
        .expect("binary builds")
        .args([
            "--quiet",
            "transcode",
            "-i",
            wav.to_str().expect("utf8 path"),
            "-o",
            out.to_str().expect("utf8 path"),
            "--threads",
            "8",
        ])
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        stderr.contains("--threads"),
        "the --threads no-effect warning must reach stderr even under --quiet, got:\n{stderr}"
    );
}
