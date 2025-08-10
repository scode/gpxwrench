## GPX Wrench — Agents Guide

This document orients future agents working on this repository. It explains the purpose, architecture, workflows, and conventions. Treat this as a living document: if anything becomes untrue or you make a substantial change, update this file immediately (see the last section).

### Project overview
GPX Wrench is a Rust CLI for processing GPX (GPS Exchange Format) data via stdin/stdout. It currently offers:
- Trim GPX track points to a user-specified time range (relative to the earliest timestamp)
- Automatically trim to the detected activity period based on speed analysis

Primary user docs and examples live in `README.md`.

### Repository layout
- `Cargo.toml`: Crate metadata and dependencies
- `src/main.rs`: CLI argument parsing and subcommand dispatch using `clap`
- `src/lib.rs`: Core types and algorithms (parsing ranges, haversine distance, speed calc, activity detection)
- `src/gpxxml.rs`: Streaming GPX read/write helpers using `quick-xml`
- `src/commands/`: Subcommand implementations
  - `trim.rs`: Implements the `trim` subcommand
  - `trim_to_activity.rs`: Implements the `trim-to-activity` subcommand
- `samples/activity.gpx`: Small sample for local testing
- `.github/workflows/ci.yml`: CI for fmt, clippy, build, and tests

### Core concepts and invariants
- I/O contract:
  - Read full GPX from stdin, write resulting GPX to stdout; print errors to stderr, exit non-zero on failure.
  - Avoid interactive prompts; be scriptable.
- Time ranges are relative to the earliest track point timestamp.
- Range semantics are inclusive of start and exclusive of end: [start, end).
- Preserve GPX structure and unknown extensions for any kept `trkpt`.
- Prefer streaming processing for XML (no full DOM); `extract_track_points` returns an in-memory list only when required by algorithms (e.g., activity detection).

### CLI commands
- `trim DUR1,DUR2` or `TS1,TS2`
  - Duration units: `s`, `m`, `h` (e.g., `5s,1m`).
  - Timestamp formats: `MM:SS` or `HH:MM:SS` (e.g., `00:15,00:45`).
  - Internally: parse with `parse_range`, read earliest time via `find_minimum_time`, then filter with `filter_xml_by_time_range`.
- `trim-to-activity [-s, --speed-threshold <m/s>] [-b, --buffer <seconds>]`
  - Detects the main activity window via speeds between consecutive track points.
  - Internals: `extract_track_points` → `detect_activity_bounds` → `filter_xml_by_time_range`.
  - Defaults: speed threshold 1.0 m/s, buffer 30 s; requires at least two timestamped points.

### Key modules and functions
- `src/lib.rs`
  - `TrackPoint { lat, lon, time }`
  - `parse_duration`, `parse_timestamp`, `parse_range`
  - `haversine_distance`, `calculate_speed`
  - `detect_activity_bounds(track_points, speed_threshold, buffer_seconds)`
- `src/gpxxml.rs`
  - `find_minimum_time(input)`
  - `filter_xml_by_time_range(input, start_time, end_time)`
  - `filter_xml_by_time_to_writer(input, start_time, end_time, writer)`
  - `extract_track_points(input)`
- `src/commands/`
  - `trim::trim_command(range_str)`
  - `trim_to_activity::trim_to_activity_command(speed_threshold, buffer)`

### Development workflow
- Build/test locally:
  - `cargo build`
  - `cargo test`
  - `cargo fmt`
  - `cargo clippy -- -D warnings`
- CI mirrors these steps and treats clippy warnings as errors. Keep the code warning-free.
- Rust edition: 2024. Dependencies: `clap`, `time`, `quick-xml` (see `Cargo.toml`).

### Adding or modifying commands
1. Create a new module under `src/commands/your_cmd.rs`.
2. Export it in `src/commands/mod.rs`.
3. Wire it into the `Commands` enum and `match` in `src/main.rs`.
4. Follow the I/O contract (stdin → stdout, stderr for errors). Prefer streaming via `src/gpxxml.rs`.
5. Update `README.md` with user-facing docs and examples.
6. Update this `AGENTS.md` with architectural notes and invariants impacted by the change.
7. Add tests (see next section), run `fmt`/`clippy`/`test`, ensure CI passes.

### Testing
- Unit tests live alongside code (e.g., `#[cfg(test)]` in `src/lib.rs` and `src/gpxxml.rs`).
- For GPX handling, prefer small inline GPX samples in tests for clarity and stability.
- Validate:
  - Range parsing (positive and negative cases)
  - Activity detection windows against controlled inputs
  - XML filtering preserves extensions and formatting for retained `trkpt`
  - Edge cases: missing timestamps, single-point tracks, out-of-order times

### Conventions and code quality
- Style: run `cargo fmt`; keep lines reasonably wrapped; avoid unrelated reformatting.
- Linting: fix all `clippy` warnings (`cargo clippy -- -D warnings`).
- Naming: descriptive function and variable names; avoid abbreviations; prefer clarity over brevity.
- Control flow: guard clauses for early exits; handle error and edge cases first; avoid deep nesting.
- Error handling: return `Result<_, Box<dyn Error>>` from command handlers; avoid panics on malformed input.

### Common pitfalls
- Forgetting that ranges are relative to the earliest timestamp, not the first element encountered.
- Off-by-one time inclusivity: ensure [start, end) behavior in filtering.
- Dropping extensions inadvertently: keep `trkpt` contents intact when within range.
- Large inputs: activity detection builds a `Vec<TrackPoint>`; consider memory implications if expanding features.

### Keeping this document up-to-date (mandatory)
- If you change behavior, interfaces, dependencies, file layout, CLI flags, or invariants, update `AGENTS.md` in the same pull request.
- If any statement here becomes untrue, fix it immediately.
- When adding major features:
  - Document new commands and their internals at a high level.
  - Note any new invariants or performance implications.
  - Cross-check `README.md` for consistency and update examples.
- Use concise, skimmable language and keep sections structured for quick onboarding.

Thank you for keeping this a high-signal guide for future agents.