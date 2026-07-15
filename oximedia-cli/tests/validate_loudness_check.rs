//! `validate --loudness-check` runs a real EBU R128 measurement;
//! `--gamut-check` warns-and-proceeds (no YUV analysis path exists yet).

mod common;

use assert_cmd::Command;

/// A -6 dBFS sine sits far above the -23 LUFS EBU R128 target, so the real
/// loudness check must report a deviation issue with the measured value.
#[test]
fn loudness_check_reports_real_deviation() {
    let (_dir, wav) = common::write_wav_fixture(1000.0, 48_000, 2, 2.0);

    let assert = Command::cargo_bin("oximedia")
        .expect("binary builds")
        .args(["validate", wav.to_str().expect("utf8"), "--loudness-check"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        stdout.contains("Loudness"),
        "the loudness check must appear in the report, got:\n{stdout}"
    );
    assert!(
        stdout.contains("LUFS"),
        "a real measured LUFS value must be reported, got:\n{stdout}"
    );
    assert!(
        stdout.contains("deviates"),
        "a hot sine must be flagged as deviating from target, got:\n{stdout}"
    );
}

/// Undecodable audio produces a visible skip issue — not a silent pass.
#[test]
fn loudness_check_undecodable_is_visible_skip() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let fake = dir.path().join("fake.wav");
    std::fs::write(&fake, b"RIFF").expect("write stub");

    let assert = Command::cargo_bin("oximedia")
        .expect("binary builds")
        .args(["validate", fake.to_str().expect("utf8"), "--loudness-check"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        stdout.contains("Loudness check skipped"),
        "the skip must be visible in the report, got:\n{stdout}"
    );
}

/// `--gamut-check` warns on stderr and proceeds (exit 0).
#[test]
fn gamut_check_warns_and_proceeds() {
    let (_dir, wav) = common::write_wav_fixture(1000.0, 48_000, 1, 0.5);

    let assert = Command::cargo_bin("oximedia")
        .expect("binary builds")
        .args(["validate", wav.to_str().expect("utf8"), "--gamut-check"])
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        stderr.contains("--gamut-check is not implemented"),
        "the gamut-check warning must reach stderr, got:\n{stderr}"
    );
}
