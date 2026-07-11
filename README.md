# xisf-header

[![CI](https://github.com/nightwatch-astro/xisf-header/actions/workflows/ci.yml/badge.svg)](https://github.com/nightwatch-astro/xisf-header/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/xisf-header.svg)](https://crates.io/crates/xisf-header)
[![Docs.rs](https://docs.rs/xisf-header/badge.svg)](https://docs.rs/xisf-header)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Read and write [XISF](https://pixinsight.com/xisf/) (Extensible Image Serialization
Format) image-file **headers** — extract the embedded FITS keywords and
create/read/update/delete the header container — built on well-chosen, pure-Rust
libraries ([`quick-xml`](https://crates.io/crates/quick-xml) and
[`thiserror`](https://crates.io/crates/thiserror), with optional
[`serde`](https://crates.io/crates/serde) support).

The crate is deliberately **header-only**: it parses and emits the 16-byte XISF
preamble plus the UTF-8 XML header, and never reads image/pixel data.

## Features

- **Parse** an XISF header from bytes or a file, validating the `XISF0100`
  signature, the little-endian XML-length field (capped at 8 MiB), and UTF-8.
- **CRUD** `FITSKeyword`s — single and bulk — with case-insensitive lookup and
  typed getters (`get_i64`, `get_f64`, `get_bool`, `get_str`).
- **`<Property>` support**, so XISF-native metadata (e.g.
  `Instrument:Telescope:FocalLength`) is available alongside the FITS keywords.
- **Write** a real, self-contained XISF container with `Header::to_bytes()` —
  ideal for generating genuine XISF fixtures.
- No `unsafe`, pure-Rust dependencies (no C/sys — MSVC-safe), MSRV 1.82.

## Install

```toml
[dependencies]
xisf-header = "0.1"
```

### Optional features

- **`serde`** — derive `Serialize`/`Deserialize` on `Header` and `FitsKeyword`:

  ```toml
  xisf-header = { version = "0.1", features = ["serde"] }
  ```

## Usage

### Parse a header and read keywords

```rust,no_run
use xisf_header::Header;

let bytes = std::fs::read("frame.xisf")?;
let header = Header::parse(&bytes)?;

// Typed getters (case-insensitive keyword lookup).
let exposure = header.get_f64("EXPTIME");
let image_type = header.get_str("IMAGETYP");

// XISF <Property> fallbacks.
let focal_length_m = header.property_f64("Instrument:Telescope:FocalLength");
# Ok::<(), xisf_header::Error>(())
```

### CRUD — single and bulk

```rust
use xisf_header::{Header, FitsKeyword};

let mut header = Header::new();

// Create (upsert): update the keyword if present, else insert it.
header.set("IMAGETYP", "Master Dark", "Type of image");
header.set("EXPTIME", "300.0", "[s] Exposure time");

// Bulk insert.
header.extend([
    FitsKeyword::new("GAIN", "100", ""),
    FitsKeyword::new("OFFSET", "50", ""),
]);

// Read.
assert_eq!(header.get_str("IMAGETYP"), Some("Master Dark"));
assert_eq!(header.get_i64("GAIN"), Some(100));

// Update.
header.set("EXPTIME", "600.0", "[s] Exposure time");

// Delete.
header.remove("OFFSET");
```

### Write a container and round-trip through a file

```rust,no_run
use xisf_header::Header;

let mut header = Header::new();
header.set("IMAGETYP", "Master Dark", "");

// Emit a real XISF container (preamble + XML header + tiny attachment).
let bytes = header.to_bytes();

// Round-trips exactly.
assert_eq!(Header::parse(&bytes)?, header);

// File helpers (reads only the header — never the pixel data).
header.write_to_file("out.xisf")?;
let reloaded = Header::read_from_file("out.xisf")?;
assert_eq!(reloaded, header);
# std::fs::remove_file("out.xisf").ok();
# Ok::<(), xisf_header::Error>(())
```

### Edit a file's header in place

```rust,no_run
use xisf_header::Header;

// Read the header, mutate it, and write the container back.
Header::update_file("out.xisf", |h| {
    h.set("OBJECT", "M31", "Target");
    h.remove("TEMP");
})?;
# Ok::<(), xisf_header::Error>(())
```

> `write_to_file`/`update_file` emit a self-contained, header-only container with a
> minimal placeholder attachment; they do not preserve an existing file's pixel
> payload. That is by design — this crate manages headers and fixtures, not image
> data.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
