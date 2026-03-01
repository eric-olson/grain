# Grain

A binary data visualizer built in Rust for reverse-engineering satellite radio protocols and other binary formats.

The primary use case is satellite data reverse-engineering — figuring out frame boundaries, sync markers, and protocol structure by visually inspecting binary data with adjustable stride (row width). Uses memory-mapped I/O and viewport rendering to stay responsive with large files.

## Features

- **2D pixel grid visualization** — view binary data as a colored image with configurable row width (stride). When the stride matches the frame length, patterns snap into aligned columns.
- **Byte and bit display modes** — grayscale byte-level view (0x00 = black, 0xFF = white) or black/white bit-level view for non-byte-aligned protocols.
- **Stride detection** — autocorrelation-based analysis that finds statistically significant periodicities in the data, suggesting likely frame lengths.
- **Sync word search** — search for hex patterns with automatic variation generation (bit-inverted, bit-reversed, byte-swapped) to find sync markers regardless of encoding quirks.
- **Memory-mapped file access** — uses `memmap2` to avoid reading entire files upfront.
- **Viewport rendering** — only the visible region is processed and rendered.
- **Background threading** — heavy operations (search, stride detection) run off the main thread so the UI stays responsive.

## Building

Requires a Rust toolchain ([rustup.rs](https://rustup.rs)).

```bash
cargo build --release
```

The binary is written to `target/release/grain`.

## Usage

```bash
cargo run --release
```

Open a file via the menu bar, then:

- Adjust the **stride** slider to reshape the 2D view until patterns align.
- Use **stride detection** to automatically find likely frame lengths.
- Switch between **byte** and **bit** display modes.
- **Search** for sync markers in hex (e.g., `1ACFFC1D`) — the tool automatically tries common variations.
- **Scroll** through the file and **zoom** in/out to inspect regions of interest.
- Hover over pixels to see offset, byte value, and position info in the status bar.

## Test Data

The `testdata/` directory contains sample binary files. Generate them with:

```bash
python testdata/generate.py
```

## Architecture

See [design.md](design.md) for the full architectural vision, including planned features like annotations, a processing pipeline, custom decoders, and a DSL.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
