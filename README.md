# GPX Wrench

> **⚠️ WARNING - DEVELOPMENT ONLY ⚠️**
> **The command line interface is subject to change without notice.**
> **This software is NOT meant to be used in production or for any critical applications.**

GPX Wrench is a command-line tool for working with GPX (GPS Exchange Format) files. At the
time of this writing functionality is extremely narrow.

## Sample Data

The repository includes `samples/activity.gpx`, a sample GPX file containing:
- **Duration**: ~110 seconds (1 minute 50 seconds)
- **Structure**: 15 seconds idle → 75 seconds cycling activity → 20 seconds idle
- **Location**: San Francisco area (Golden Gate Park vicinity)
- **Activity**: Simulated cycling with realistic speed progression

This file is perfect for testing both commands and demonstrates the difference between manual trimming and automatic activity detection.

## Usage

GPX Wrench uses subcommands to organize its functionality. A sample GPX file is provided in `samples/activity.gpx` for testing.

```bash
# Show available commands
cargo run -- --help

# Show trim command help
cargo run -- trim --help

# Try the sample file
cat samples/activity.gpx | cargo run -- trim-to-activity
```

### Trim Command

The `trim` command filters GPX track points based on time ranges. You can specify ranges using either duration format or timestamp format.

#### Duration Format Examples

Duration format uses `DUR1,DUR2` where each duration is a number followed by `s` (seconds), `m` (minutes), or `h` (hours). The range is relative to the earliest timestamp in the GPX file.

```bash
# Keep data from 0 seconds to 30 seconds after the start (using sample file)
cat samples/activity.gpx | cargo run -- trim 0s,30s > output.gpx

# Keep data from 5 seconds to 1 minute after the start
cat samples/activity.gpx | cargo run -- trim 5s,1m > output.gpx

# Keep data from 15 seconds to 45 seconds (covers the main activity)
cat samples/activity.gpx | cargo run -- trim 15s,45s > output.gpx

# You can also use your own GPX files
cat your-track.gpx | cargo run -- trim 1h,2h > output.gpx
```

#### Timestamp Format Examples

Timestamp format uses `TS1,TS2` where each timestamp is in `MM:SS` or `HH:MM:SS` format. These are also relative to the earliest timestamp in the GPX file.

```bash
# Keep data from 0:05 (5 seconds) to 1:30 (1 minute 30 seconds) after start
cat samples/activity.gpx | cargo run -- trim 00:05,01:30 > output.gpx

# Keep data from 0:15 to 0:45 (main activity period)
cat samples/activity.gpx | cargo run -- trim 00:15,00:45 > output.gpx

# You can also use longer timestamp formats for longer tracks
cat longer-track.gpx | cargo run -- trim 01:02:30,02:15:45 > output.gpx
```

#### Important Notes

- Both duration and timestamp formats specify ranges relative to the **earliest timestamp** in the GPX file
- The range is inclusive of the start time and exclusive of the end time `[start, end)`
- All GPX extensions (including heart rate data) are preserved in the filtered output
- Track points without timestamps are excluded from the output

### Trim-to-Activity Command

The `trim-to-activity` command automatically detects periods of activity in GPX tracks based on speed analysis and trims the track to include only the main activity period. This is useful for removing stationary periods before and after activities like cycling, running, or motorcycling.

#### How It Works

1. **Speed Analysis**: Calculates speed between consecutive GPS points using the haversine formula
2. **Activity Detection**: Identifies periods where speed consistently exceeds the threshold
3. **Conservative Trimming**: Adds buffer time before/after detected activity to avoid cutting off important data
4. **Single Activity**: Designed for tracks with one main activity period

#### Basic Usage

```bash
# Use default settings (1.0 m/s speed threshold, 30-second buffer) with sample file
cat samples/activity.gpx | cargo run -- trim-to-activity > output.gpx

# Show available options
cargo run -- trim-to-activity --help
```

#### Advanced Usage

```bash
# Higher speed threshold for cycling (3 m/s = ~11 km/h) with sample file
cat samples/activity.gpx | cargo run -- trim-to-activity --speed-threshold 3.0 > output.gpx

# Shorter buffer for precise trimming (5 seconds before/after activity)
cat samples/activity.gpx | cargo run -- trim-to-activity --buffer 5 > output.gpx

# Combined: higher threshold with minimal buffer
cat samples/activity.gpx | cargo run -- trim-to-activity -s 2.5 -b 5 > output.gpx
```

#### Parameters

- `--speed-threshold` / `-s`: Minimum speed in m/s to consider as activity (default: 1.0)
- `--buffer` / `-b`: Buffer time in seconds to add before/after detected activity (default: 30)

#### Examples by Activity Type

```bash
# Walking/hiking (low speed threshold)
cat hike.gpx | cargo run -- trim-to-activity -s 0.5 -b 60 > output.gpx

# Cycling (moderate speed threshold) 
cat bike.gpx | cargo run -- trim-to-activity -s 2.0 -b 30 > output.gpx

# Driving/motorcycling (high speed threshold)
cat drive.gpx | cargo run -- trim-to-activity -s 5.0 -b 15 > output.gpx
```

#### Important Notes

- Requires at least 2 track points with valid coordinates and timestamps
- Uses conservative detection (requires 3+ consecutive points above threshold)
- Errs on the side of inclusion - better to keep too much than cut off activity
- Preserves all GPX extensions and formatting like the `trim` command

## Development

```bash
cargo build
cargo test
cargo fmt
cargo clippy
```
