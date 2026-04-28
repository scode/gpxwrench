use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use time::OffsetDateTime;

fn sample_gpx() -> &'static str {
    include_str!("../samples/activity.gpx")
}

fn parse_timestamp(timestamp: &str) -> gpx::Time {
    OffsetDateTime::parse(
        timestamp,
        &time::format_description::well_known::Iso8601::DEFAULT,
    )
    .unwrap()
    .into()
}

#[test]
fn test_trim_command_duration_range() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    let output = cmd
        .arg("trim")
        .arg("10s,40s")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .stdout(predicate::str::contains("<gpx"))
        .stdout(predicate::str::contains("</gpx>"))
        .stdout(predicate::str::contains("<trkpt"))
        .get_output()
        .stdout
        .clone();

    let gpx: gpx::Gpx = gpx::read(output.as_slice()).unwrap();
    let points = &gpx.tracks[0].segments[0].points;
    let times: Vec<_> = points.iter().map(|point| point.time.unwrap()).collect();

    assert_eq!(
        times.first(),
        Some(&parse_timestamp("2023-06-15T10:00:10Z"))
    );
    assert_eq!(times.last(), Some(&parse_timestamp("2023-06-15T10:00:35Z")));
    assert!(!times.contains(&parse_timestamp("2023-06-15T10:00:40Z")));
}

#[test]
fn test_trim_command_timestamp_range() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    let output = cmd
        .arg("trim")
        .arg("00:10,00:40")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .stdout(predicate::str::contains("<gpx"))
        .stdout(predicate::str::contains("</gpx>"))
        .get_output()
        .stdout
        .clone();

    let gpx: gpx::Gpx = gpx::read(output.as_slice()).unwrap();
    let points = &gpx.tracks[0].segments[0].points;
    let times: Vec<_> = points.iter().map(|point| point.time.unwrap()).collect();

    assert_eq!(
        times.first(),
        Some(&parse_timestamp("2023-06-15T10:00:10Z"))
    );
    assert_eq!(times.last(), Some(&parse_timestamp("2023-06-15T10:00:35Z")));
    assert!(!times.contains(&parse_timestamp("2023-06-15T10:00:40Z")));
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
    let points = &trimmed_gpx.tracks[0].segments[0].points;
    let trimmed_count = points.len();

    // With idle portions at start/end removed, should have fewer points
    assert!(
        trimmed_count < full_count,
        "Activity-trimmed GPX should have fewer points: {} < {}",
        trimmed_count,
        full_count
    );
    assert_eq!(
        points.first().and_then(|point| point.time),
        Some(parse_timestamp("2023-06-15T10:00:05Z"))
    );
    assert_eq!(
        points.last().and_then(|point| point.time),
        Some(parse_timestamp("2023-06-15T10:01:35Z"))
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
fn test_trim_to_activity_without_valid_points_fails() {
    let gpx = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="37.7749" lon="-122.4194"/>
    </trkseg>
  </trk>
</gpx>"#;

    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim-to-activity")
        .write_stdin(gpx)
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least 2 track points"));
}

#[test]
fn test_trim_to_activity_without_valid_coordinates_fails() {
    let gpx = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="37.7749">
        <time>2023-01-01T10:00:00Z</time>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim-to-activity")
        .write_stdin(gpx)
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least 2 track points"));
}

#[test]
fn test_trim_to_activity_rejects_invalid_speed_thresholds() {
    for threshold in ["-1.0", "NaN", "inf"] {
        let mut cmd = cargo_bin_cmd!("gpxwrench");
        cmd.arg("trim-to-activity")
            .arg(format!("--speed-threshold={threshold}"))
            .write_stdin(sample_gpx())
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "Speed threshold must be a finite non-negative number",
            ));
    }
}

#[test]
fn test_trim_to_activity_accepts_zero_speed_threshold() {
    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim-to-activity")
        .arg("--speed-threshold=0.0")
        .write_stdin(sample_gpx())
        .assert()
        .success()
        .stdout(predicate::str::contains("<gpx"));
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
fn test_trim_excludes_points_when_no_timestamps_exist() {
    let gpx = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="37.7749" lon="-122.4194"/>
    </trkseg>
  </trk>
</gpx>"#;

    let mut cmd = cargo_bin_cmd!("gpxwrench");
    let output = cmd
        .arg("trim")
        .arg("0s,1s")
        .write_stdin(gpx)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let gpx: gpx::Gpx = gpx::read(output.as_slice()).unwrap();
    assert_eq!(gpx.tracks[0].segments[0].points.len(), 0);
}

#[test]
fn test_trim_excludes_points_when_no_valid_timestamps_exist() {
    let gpx = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="37.7749" lon="-122.4194">
        <time>not-a-time</time>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    let mut cmd = cargo_bin_cmd!("gpxwrench");
    let output = cmd
        .arg("trim")
        .arg("0s,1s")
        .write_stdin(gpx)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let gpx: gpx::Gpx = gpx::read(output.as_slice()).unwrap();
    assert_eq!(gpx.tracks[0].segments[0].points.len(), 0);
}

#[test]
fn test_trim_timestamp_overflow_fails() {
    let gpx = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="37.7749" lon="-122.4194">
        <time>9999-12-31T23:59:59.999999999Z</time>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim")
        .arg("1s,2s")
        .write_stdin(gpx)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Trim start exceeds"));
}

#[test]
fn test_trim_end_timestamp_overflow_fails() {
    let gpx = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="37.7749" lon="-122.4194">
        <time>9999-12-31T23:59:58.999999999Z</time>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    let mut cmd = cargo_bin_cmd!("gpxwrench");
    cmd.arg("trim")
        .arg("0s,2s")
        .write_stdin(gpx)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Trim end exceeds"));
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
