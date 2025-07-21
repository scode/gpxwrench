# GPX Wrench

> **⚠️ WARNING - DEVELOPMENT ONLY ⚠️**
> **The command line interface is subject to change without notice.**
> **This software is NOT meant to be used in production or for any critical applications.**

GPX Wrench is a command-line tool for working with GPX (GPS Exchange Format) files. At the
time of this writing functionality is extremely narrow.

## Usage

```bash
cat input.gpx | cargo run > output.gpx
```

## Development

```bash
cargo build
cargo test
cargo fmt
cargo clippy
```
