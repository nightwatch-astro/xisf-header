# xisf-header

[![CI](https://github.com/nightwatch-astro/xisf-header/actions/workflows/ci.yml/badge.svg)](https://github.com/nightwatch-astro/xisf-header/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/xisf-header.svg)](https://crates.io/crates/xisf-header)
[![Docs.rs](https://docs.rs/xisf-header/badge.svg)](https://docs.rs/xisf-header)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/nightwatch-astro/xisf-header/blob/main/LICENSE)

Rust crate that reads and writes [XISF](https://pixinsight.com/xisf/)
(Extensible Image Serialization Format) image-file headers: it extracts the
embedded FITS keywords and XISF `<Property>` elements, supports
create/read/update/delete on both, and serializes a header back into an XISF
container.

The crate is header-only: it parses and emits the 16-byte XISF preamble plus
the UTF-8 XML header, and never reads image/pixel data.

## Features

- **Parse** an XISF header from bytes or a file. The `XISF0100` signature, the
  little-endian XML-length field (capped at 8 MiB), and UTF-8 encoding are
  validated.
- **Strict keyword access.** A bare name must be unique or the accessor returns
  `Error::Ambiguous`; repeated keywords (e.g. `HISTORY`) are addressed with an
  `(name, n)` key or the `get_all`/`count` helpers.
- **Typed reads and writes.** One generic `get::<T>` over the open
  [`FromField`] trait (`String`, `f64`, `i64`, `u32`, `bool`, and a date/time),
  with `get_str`/`get_f64`/… wrappers; writes take `impl IntoValue`, so the Rust
  type chooses string vs. bare-literal formatting.
- **`<Property>` round-trip.** XISF properties keep their `type`, `comment`,
  and `format` attributes verbatim; `String` properties stored as child text
  are read. Values are stored raw (XISF properties are not FITS-quoted).
- **Two serialization outputs.** `to_bytes(&hints)` for a self-contained
  container and `to_header_bytes(&hints)` for the header block alone.
- No `unsafe`. Dependencies are pure Rust (no C/sys crates): `quick-xml`,
  `thiserror`, `time`, and optional `serde`. MSRV 1.82.

## Install

```toml
[dependencies]
xisf-header = "0.2"
```

### Optional features

- **`serde`** — derive `Serialize`/`Deserialize` on `Header`, `FitsKeyword`,
  `Property`, and the value types:

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

### XISF properties

```rust
use xisf_header::Header;

let mut header = Header::new();

// Plain `set_property` creates a `String` property; an explicit XISF type is
// kept on the property and survives round-trips.
header.set_property("Observation:Object:Name", "NGC 7000")?;
header.set_property_with_type("Instrument:Telescope:FocalLength", "0.53", "Float32")?;

assert_eq!(header.property("Observation:Object:Name"), Some("NGC 7000"));
assert_eq!(header.property_get::<f64>("Instrument:Telescope:FocalLength"), Some(0.53));
assert_eq!(header.properties()["Instrument:Telescope:FocalLength"].type_, "Float32");
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

> **Warning:** `to_bytes`/`write_to_file`/`update_file` emit a self-contained,
> header-only container: the data block is **zero-filled** from
> `StructuralHints`, and XML elements the crate does not model (`Metadata`,
> `Resolution`, thumbnails, …) are not re-emitted. Do not point them at files
> whose pixel data must be kept. To edit a real image's header, emit
> `to_header_bytes(&hints)` and append the file's original data yourself.

## Documentation

Full API documentation is generated from the source doc comments and published
at **[docs.rs/xisf-header](https://docs.rs/xisf-header)** for every release
(all features enabled). Build it locally with `cargo doc --no-deps
--all-features --open`. Every public item is documented; CI fails the build on
missing or broken documentation.

## License

Licensed under the
[Apache License, Version 2.0](https://github.com/nightwatch-astro/xisf-header/blob/main/LICENSE).
