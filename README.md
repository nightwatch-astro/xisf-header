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

- **Parse** an XISF header from bytes or a file with
  [`Header::parse`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.parse)
  /
  [`Header::read_from_file`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.read_from_file).
  The `XISF0100` signature, the little-endian XML-length field (capped at 8
  MiB), and UTF-8 encoding are validated.
- **Strict keyword access.** A bare name must be unique or the accessor
  returns
  [`Error::Ambiguous`](https://docs.rs/xisf-header/latest/xisf_header/enum.Error.html#variant.Ambiguous);
  repeated keywords (e.g. `HISTORY`) are addressed with an
  [`(name, n)` key](https://docs.rs/xisf-header/latest/xisf_header/enum.Key.html)
  or the
  [`get_all`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.get_all)/[`count`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.count)
  helpers.
- **Typed reads and writes.** One generic
  [`get::<T>`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.get)
  over the open
  [`FromField`](https://docs.rs/xisf-header/latest/xisf_header/trait.FromField.html)
  trait (`String`, `f64`, `i64`, `u32`, `bool`, and a date/time), with
  `get_str`/`get_f64`/… wrappers; writes take
  [`impl IntoValue`](https://docs.rs/xisf-header/latest/xisf_header/trait.IntoValue.html),
  so the Rust type chooses string vs. bare-literal formatting.
- **[`<Property>`](https://docs.rs/xisf-header/latest/xisf_header/struct.Property.html)
  round-trip.** XISF properties keep their `type`, `comment`, and `format`
  attributes verbatim. A `String` property written as child text
  (`<Property id=…>text</Property>`) is read the same as the attribute form,
  and writes normalize it to a `value=` attribute. Values are stored raw
  (XISF properties are not FITS-quoted).
- **Two write paths.** Edit an existing file in place with
  [`update_file`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.update_file)
  — the common case — which splices only the changed keywords/properties into
  the file's raw bytes (byte-exact and data-preserving). To create a **new**
  file from pixel data you already have, use
  [`write_to_file`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.write_to_file)
  (errors rather than overwriting an existing path); or emit just the header
  block with
  [`to_header_bytes(&hints)`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.to_header_bytes)
  for full control over assembly.
- **Enumerate and bulk-edit.** Read keywords in document order with
  [`keywords`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.keywords)/[`iter`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.iter);
  apply atomic batches with
  [`set_many`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.set_many)/[`remove_many`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.remove_many),
  and clear every occurrence of a repeated name with
  [`remove_all`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.remove_all).
- No `unsafe`. Dependencies are pure Rust (no C/sys crates): `quick-xml`,
  `thiserror`, `time`, and optional `serde`. MSRV 1.82.

## Install

```toml
[dependencies]
xisf-header = "0.2"
```

### Optional features

- **`serde`** — derive `Serialize`/`Deserialize` on `Header`,
  [`FitsKeyword`](https://docs.rs/xisf-header/latest/xisf_header/struct.FitsKeyword.html),
  `Property`, and the value types:

  ```toml
  xisf-header = { version = "0.2", features = ["serde"] }
  ```

## Usage

### Parse a header and read keywords

[`Header::parse`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.parse)
reads a byte buffer into a
[`Header`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html).

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

[`Header::new`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.new)
starts empty;
[`set`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.set),
[`set_comment`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.set_comment),
[`set_with_comment`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.set_with_comment),
and
[`remove`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.remove)
edit it in place.

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

[`append`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.append)
adds an occurrence unconditionally; read every occurrence with
[`get_all`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.get_all),
and select, update, or remove one with an
[`(name, n)`](https://docs.rs/xisf-header/latest/xisf_header/enum.Key.html)
key.

```rust
use xisf_header::Header;

let mut header = Header::new();
header.append("HISTORY", "reduced with siril")?;
header.append("HISTORY", "stacked 20x300s")?;

// A bare name is ambiguous once it repeats — select an occurrence instead.
assert!(header.get_str("HISTORY").is_err());
assert_eq!(header.get_str(("HISTORY", 1))?, Some("stacked 20x300s"));
assert_eq!(header.count("HISTORY"), 2);

// Read every occurrence in order.
assert_eq!(
    header.get_all::<String>("HISTORY"),
    ["reduced with siril", "stacked 20x300s"],
);
// Update one occurrence in place.
header.set(("HISTORY", 0), "reduced with siril v2")?;
assert_eq!(header.get_str(("HISTORY", 0))?, Some("reduced with siril v2"));
// Remove one occurrence (or clear them all with `remove_all`).
header.remove(("HISTORY", 0))?;
assert_eq!(header.count("HISTORY"), 1);
# Ok::<(), xisf_header::Error>(())
```

### FITS keywords vs XISF properties

Two different metadata namespaces:

- **FITS keywords** (`set`/`get`/`append`) — embedded `<FITSKeyword name=…
  value=… comment=…/>` elements carried over for FITS compatibility. Names
  are ≤ 8 uppercase ASCII characters and can repeat (e.g. `HISTORY`). Use
  these for metadata other FITS-aware tools need to read.
- **XISF properties** (`set_property`/`property`/`set_property_with_type`) —
  native XISF `<Property>` elements. Ids are colon-delimited and hierarchical
  (e.g. `Observation:Object:Name`), carry an explicit XISF type, and are
  unique per id. Use these for XISF-native structured metadata.

### XISF properties

[`set_property`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.set_property)
and
[`set_property_with_type`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.set_property_with_type)
write a
[`Property`](https://docs.rs/xisf-header/latest/xisf_header/struct.Property.html)
entry;
[`remove_property`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.remove_property)
deletes one by id.

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

[`Fixed`](https://docs.rs/xisf-header/latest/xisf_header/struct.Fixed.html)
and
[`Sci`](https://docs.rs/xisf-header/latest/xisf_header/struct.Sci.html)
wrap an `f64` for fixed-point or scientific-notation output; both implement
[`IntoValue`](https://docs.rs/xisf-header/latest/xisf_header/trait.IntoValue.html).

```rust
use xisf_header::{Fixed, Header};

let mut header = Header::new();
header.set("EXPTIME", Fixed(300.0, 2))?; // fixed-point, 2 decimals
assert_eq!(header.get_str("EXPTIME")?, Some("300.00"));
# Ok::<(), xisf_header::Error>(())
```

### Edit a file's header in place

The common path: change a keyword or property on an existing XISF file
without touching its pixel data or unmodeled XML.

[`Header::update_file`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.update_file)
reads a file's header, applies an edit closure, and splices the result back
into the file. It is byte-exact and data-preserving: everything outside the
edited `<FITSKeyword>`/`<Property>` elements — unmodeled XML (`Metadata`,
`Resolution`, thumbnails, …), whitespace, and the attached pixel data —
survives untouched, and a no-op edit reproduces the file byte-for-byte. If an
edit changes the header's length, the `<Image location>` offset is
recomputed and the original data is moved (unchanged) to the new offset.

```rust,no_run
use xisf_header::Header;

Header::update_file("out.xisf", |h| {
    h.set("OBJECT", "NGC 7000")?;
    Ok(())
})?;
# Ok::<(), xisf_header::Error>(())
```

`update_file` targets the common single-image layout: exactly one `<Image
location="attachment:…">` element. A file with zero or multiple attachments
(e.g. a `Thumbnail` alongside the `Image`) is rejected with
[`Error::Unsupported`](https://docs.rs/xisf-header/latest/xisf_header/enum.Error.html#variant.Unsupported)
rather than risking data loss.

### Create a new file

Use this only when you're assembling a fresh XISF file from pixel data you
already have — not for editing one that exists (that's `update_file` above).
[`Header::write_to_file`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.write_to_file)
writes the preamble, the XML header (`<Image>` filled in from
[`StructuralHints`](https://docs.rs/xisf-header/latest/xisf_header/struct.StructuralHints.html)),
and your `data` bytes to a **new** file; it errors if `path` already exists
rather than overwriting it, and never fabricates pixel data — `data` is
always the caller's own.

```rust,no_run
use xisf_header::{Header, StructuralHints};

let mut header = Header::new();
header.set("IMAGETYP", "Master Dark")?;

let hints = StructuralHints::default(); // 1x1x1 8-bit grayscale = 1 byte
let data = [0u8]; // the caller's own pixel data
header.write_to_file("out.xisf", &hints, &data)?;

let reloaded = Header::read_from_file("out.xisf")?;
assert_eq!(reloaded, header);
# std::fs::remove_file("out.xisf").ok();
# Ok::<(), xisf_header::Error>(())
```

## Documentation

- **[Quickstart guide](https://docs.rs/xisf-header/latest/xisf_header/guide/index.html)** — a task-oriented walkthrough backed
  by [`examples/quickstart.rs`](https://github.com/nightwatch-astro/xisf-header/blob/main/examples/quickstart.rs).
- **[docs.rs/xisf-header](https://docs.rs/xisf-header)** — full API
  documentation generated from the source doc comments, published for every
  release (all features enabled). Build it locally with `cargo doc --no-deps
  --all-features --open`. Every public item is documented; CI fails the build
  on missing or broken documentation.

## License

[![License: MPL 2.0](https://img.shields.io/badge/License-MPL_2.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)

This project is licensed under the Mozilla Public License 2.0 — see [LICENSE](./LICENSE) for details.

You can use this library in closed-source projects. If you modify any of the source files in this library, the modified files must be made available under the MPL-2.0 when distributed.
