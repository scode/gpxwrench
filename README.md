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

# Trim GPX track points to keep only those within the first 5 seconds
cat input.gpx | cargo run -- trim > output.gpx
```

### Available Commands

- `trim`: Filters GPX track points to keep only those within the first 5 seconds of the earliest timestamp, preserving all extensions including heart rate data

## Development

```bash
cargo build
cargo test
cargo fmt
cargo clippy
```
