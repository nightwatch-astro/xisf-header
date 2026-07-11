# xisf-header

[![CI](https://github.com/nightwatch-astro/xisf-header/actions/workflows/ci.yml/badge.svg)](https://github.com/nightwatch-astro/xisf-header/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/xisf-header.svg)](https://crates.io/crates/xisf-header)
[![Docs.rs](https://docs.rs/xisf-header/badge.svg)](https://docs.rs/xisf-header)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Read and write [XISF](https://pixinsight.com/xisf/) (Extensible Image Serialization
Format) image-file **headers** — extract the embedded FITS keywords and
create/read/update/delete the header container — built on well-chosen, pure-Rust
libraries ([`quick-xml`](https://crates.io/crates/quick-xml),
[`thiserror`](https://crates.io/crates/thiserror), and
[`time`](https://crates.io/crates/time), with optional
[`serde`](https://crates.io/crates/serde) support).

The crate is deliberately **header-only**: it parses and emits the 16-byte XISF
preamble plus the UTF-8 XML header, and never reads image/pixel data.

## Features

- **Parse** an XISF header from bytes or a file, validating the `XISF0100`
  signature, the little-endian XML-length field (capped at 8 MiB), and UTF-8.
- **Strict, keyword-oriented access.** A bare name must be unique or the accessor
  returns [`Error::Ambiguous`]; repeated keywords (e.g. `HISTORY`) are reached with
  an `(name, n)` key or the `get_all`/`count` helpers. No silent first-wins.
- **Typed reads and writes.** One generic `get::<T>` over the open
  [`FromField`] trait (`String`, `f64`, `i64`, `u32`, `bool`, and a date/time),
  with `get_str`/`get_f64`/… wrappers; writes take `impl IntoValue`, so the Rust
  type chooses string vs. bare-literal formatting.
- **`<Property>` support**, so XISF-native metadata (e.g.
  `Instrument:Telescope:FocalLength`) is available alongside the FITS keywords.
- **Two serialization outputs.** `to_bytes(&hints)` for a self-contained container
  and `to_header_bytes(&hints)` for the header block alone.
- No `unsafe`, pure-Rust dependencies (no C/sys — MSVC-safe), MSRV 1.82.

## Install

```toml
[dependencies]
xisf-header = "0.2"
```

### Optional features

- **`serde`** — derive `Serialize`/`Deserialize` on `Header`, `FitsKeyword`, and
  the value types:

  ```toml
  xisf-header = { version = "0.2", features = ["serde"] }
  ```

## Usage

### Parse a header and read keywords

```rust,no_run
use xisf_header::Header;

let bytes = std::fs::read("frame.xisf")?;
let header = Header::parse(&bytes)?;

// Typed reads are strict: `?` surfaces a duplicate-keyword ambiguity; the inner
// `Option` is `None` when the keyword is absent or unreadable as that type.
let exposure = header.get_f64("EXPTIME")?;
let image_type = header.get_str("IMAGETYP")?;

// XISF <Property> access.
let focal_length_m = header.property_get::<f64>("Instrument:Telescope:FocalLength");
# Ok::<(), xisf_header::Error>(())
```

### Create, read, update, delete

```rust
use xisf_header::Header;

let mut header = Header::new();

// `set` upserts: update a unique keyword in place, or append when absent. The
// Rust type of the value chooses its on-disk form — strings are quoted,
// numbers and logicals are bare literals.
header.set("IMAGETYP", "Master Dark")?;
header.set_comment("IMAGETYP", "Type of image")?;
header.set("EXPTIME", 300.0)?;
header.set("GAIN", 100_i64)?;

assert_eq!(header.get_str("IMAGETYP")?, Some("Master Dark"));
assert_eq!(header.get_i64("GAIN")?, Some(100));

header.set("EXPTIME", 600.0)?; // update
header.remove("GAIN")?; // delete
# Ok::<(), xisf_header::Error>(())
```

### Repeated keywords

```rust
use xisf_header::Header;

let mut header = Header::new();
header.append("HISTORY", "reduced with siril")?;
header.append("HISTORY", "stacked 20x300s")?;

// A bare name is ambiguous once it repeats — select an occurrence instead.
assert!(header.get_str("HISTORY").is_err());
assert_eq!(header.get_str(("HISTORY", 1))?, Some("stacked 20x300s"));
assert_eq!(header.count("HISTORY"), 2);
# Ok::<(), xisf_header::Error>(())
```

### Controlled numeric formatting

```rust
use xisf_header::{Fixed, Header};

let mut header = Header::new();
header.set("EXPTIME", Fixed(300.0, 2))?; // fixed-point, 2 decimals
assert_eq!(header.get_str("EXPTIME")?, Some("300.00"));
# Ok::<(), xisf_header::Error>(())
```

### Write a container and round-trip through a file

```rust,no_run
use xisf_header::{Header, StructuralHints};

let mut header = Header::new();
header.set("IMAGETYP", "Master Dark")?;

let hints = StructuralHints::default();

// Emit a complete container, or just the header block.
let bytes = header.to_bytes(&hints);
assert_eq!(Header::parse(&bytes)?, header);

header.write_to_file("out.xisf", &hints)?;
let reloaded = Header::read_from_file("out.xisf")?;
assert_eq!(reloaded, header);
# std::fs::remove_file("out.xisf").ok();
# Ok::<(), xisf_header::Error>(())
```

### Edit a file's header in place

```rust,no_run
use xisf_header::{Header, StructuralHints};

Header::update_file("out.xisf", &StructuralHints::default(), |h| {
    h.set("OBJECT", "M31").unwrap();
    h.remove("TEMP").unwrap();
})?;
# Ok::<(), xisf_header::Error>(())
```

> `to_bytes`/`write_to_file`/`update_file` emit a self-contained, header-only
> container with a data block sized from `StructuralHints`; they do not preserve an
> existing file's pixel payload. That is by design — this crate manages headers and
> fixtures, not image data.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
