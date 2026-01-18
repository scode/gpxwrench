use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

fn sample_gpx() -> &'static str {
    include_str!("../samples/activity.gpx")
}

#[test]
fn test_trim_command_duration_range() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim")
        .arg("10s,40s")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .stdout(predicate::str::contains("<gpx"))
        .stdout(predicate::str::contains("</gpx>"))
        .stdout(predicate::str::contains("<trkpt"));
}

#[test]
fn test_trim_command_timestamp_range() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim")
        .arg("00:10,00:40")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .stdout(predicate::str::contains("<gpx"))
        .stdout(predicate::str::contains("</gpx>"));
}

#[test]
fn test_trim_command_output_is_valid_gpx() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    let output = cmd
        .arg("trim")
        .arg("15s,60s")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Verify output parses as valid GPX
    let gpx_result: Result<gpx::Gpx, _> = gpx::read(output.as_slice());
    assert!(gpx_result.is_ok(), "Output should be valid GPX");

    let gpx = gpx_result.unwrap();
    assert_eq!(gpx.tracks.len(), 1);
    assert!(!gpx.tracks[0].segments[0].points.is_empty());
}

#[test]
fn test_trim_command_reduces_point_count() {
    // First, get point count from full file
    let full_gpx: gpx::Gpx = gpx::read(sample_gpx().as_bytes()).unwrap();
    let full_count = full_gpx.tracks[0].segments[0].points.len();

    // Trim to a subset
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    let output = cmd
        .arg("trim")
        .arg("20s,50s")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let trimmed_gpx: gpx::Gpx = gpx::read(output.as_slice()).unwrap();
    let trimmed_count = trimmed_gpx.tracks[0].segments[0].points.len();

    assert!(
        trimmed_count < full_count,
        "Trimmed GPX should have fewer points: {} < {}",
        trimmed_count,
        full_count
    );
}

#[test]
fn test_trim_to_activity_command_default_params() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim-to-activity")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .stdout(predicate::str::contains("<gpx"))
        .stdout(predicate::str::contains("</gpx>"));
}

#[test]
fn test_trim_to_activity_command_custom_speed() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim-to-activity")
        .arg("--speed-threshold")
        .arg("5.0")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .stdout(predicate::str::contains("<gpx"));
}

#[test]
fn test_trim_to_activity_command_custom_buffer() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim-to-activity")
        .arg("--buffer")
        .arg("60")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .stdout(predicate::str::contains("<gpx"));
}

#[test]
fn test_trim_to_activity_removes_idle_portions() {
    // The sample GPX has idle time at start and end
    let full_gpx: gpx::Gpx = gpx::read(sample_gpx().as_bytes()).unwrap();
    let full_count = full_gpx.tracks[0].segments[0].points.len();

    let mut cmd = cargo_bin_cmd!("gpxwrench");
    let output = cmd
        .arg("trim-to-activity")
        .arg("--speed-threshold")
        .arg("1.0")
        .arg("--buffer")
        .arg("0")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let trimmed_gpx: gpx::Gpx = gpx::read(output.as_slice()).unwrap();
    let trimmed_count = trimmed_gpx.tracks[0].segments[0].points.len();

    // With idle portions at start/end removed, should have fewer points
    assert!(
        trimmed_count < full_count,
        "Activity-trimmed GPX should have fewer points: {} < {}",
        trimmed_count,
        full_count
    );
}

#[test]
fn test_trim_to_activity_output_is_valid_gpx() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    let output = cmd
        .arg("trim-to-activity")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let gpx_result: Result<gpx::Gpx, _> = gpx::read(output.as_slice());
    assert!(gpx_result.is_ok(), "Output should be valid GPX");
}

#[test]
fn test_trim_invalid_range_fails() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim")
        .arg("invalid")
        .write_stdin(sample_gpx())
        .assert()
        .failure();
}

#[test]
fn test_trim_command_preserves_gpx_structure() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    let output = cmd
        .arg("trim")
        .arg("10s,90s")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let gpx: gpx::Gpx = gpx::read(output.as_slice()).unwrap();

    // Verify GPX structure is preserved
    assert_eq!(gpx.tracks.len(), 1, "Should have one track");
    assert_eq!(gpx.tracks[0].segments.len(), 1, "Should have one segment");

    // Verify points have required data
    for point in &gpx.tracks[0].segments[0].points {
        assert!(point.time.is_some(), "Each point should have a time");
    }
}
