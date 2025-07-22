# GPX Wrench

> **⚠️ WARNING - DEVELOPMENT ONLY ⚠️**
> **The command line interface is subject to change without notice.**
> **This software is NOT meant to be used in production or for any critical applications.**

GPX Wrench is a command-line tool for working with GPX (GPS Exchange Format) files. At the
time of this writing functionality is extremely narrow.

## Usage

GPX Wrench uses subcommands to organize its functionality:

```bash
# Show available commands
cargo run -- --help

# Show trim command help
cargo run -- trim --help
```

### Trim Command

The `trim` command filters GPX track points based on time ranges. You can specify ranges using either duration format or timestamp format.

#### Duration Format Examples

Duration format uses `DUR1,DUR2` where each duration is a number followed by `s` (seconds), `m` (minutes), or `h` (hours). The range is relative to the earliest timestamp in the GPX file.

```bash
# Keep data from 0 seconds to 30 seconds after the start
cat input.gpx | cargo run -- trim 0s,30s > output.gpx

# Keep data from 5 seconds to 1 minute after the start
cat input.gpx | cargo run -- trim 5s,1m > output.gpx

# Keep data from 2 minutes to 5 minutes after the start
cat input.gpx | cargo run -- trim 2m,5m > output.gpx

# Keep data from 1 hour to 2 hours after the start
cat input.gpx | cargo run -- trim 1h,2h > output.gpx
```

#### Timestamp Format Examples

Timestamp format uses `TS1,TS2` where each timestamp is in `MM:SS` or `HH:MM:SS` format. These are also relative to the earliest timestamp in the GPX file.

```bash
# Keep data from 0:05 (5 seconds) to 1:30 (1 minute 30 seconds) after start
cat input.gpx | cargo run -- trim 00:05,01:30 > output.gpx

# Keep data from 1:00 to 5:00 (1 minute to 5 minutes) after start
cat input.gpx | cargo run -- trim 01:00,05:00 > output.gpx

# Keep data from 1:02:30 to 2:15:45 after start
cat input.gpx | cargo run -- trim 01:02:30,02:15:45 > output.gpx
```

#### Important Notes

- Both duration and timestamp formats specify ranges relative to the **earliest timestamp** in the GPX file
- The range is inclusive of the start time and exclusive of the end time `[start, end)`
- All GPX extensions (including heart rate data) are preserved in the filtered output
- Track points without timestamps are excluded from the output

## Development

```bash
cargo build
cargo test
cargo fmt
cargo clippy
```
