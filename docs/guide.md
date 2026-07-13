# Quickstart guide

A task-oriented walkthrough for building, editing, and round-tripping an XISF
header. The snippets below mirror
[`examples/quickstart.rs`](../examples/quickstart.rs) (`cargo run --example
quickstart`), which runs the same steps end to end against a temporary file.

The walkthrough builds one header for a master dark calibration frame and
carries it through every capability in this crate.

## Create a header and set FITS keywords

[`Header::new`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.new)
starts empty.
[`Header::set`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.set)
upserts a keyword: it appends when the name is absent and updates in place
when it is unique. The Rust type of the value chooses its on-disk form —
strings are quoted, numbers are bare literals; wrap a float in
[`Fixed`](https://docs.rs/xisf-header/latest/xisf_header/struct.Fixed.html)
for controlled fixed-point formatting.

```rust,no_run
use xisf_header::{Fixed, Header, StructuralHints};

let mut header = Header::new();
header.set("IMAGETYP", "Master Dark")?;
header.set_comment("IMAGETYP", "Type of image")?;
header.set("EXPTIME", Fixed(300.0, 2))?; // fixed-point, 2 decimals
header.set("GAIN", 100_i64)?;
# Ok::<(), xisf_header::Error>(())
```

## Track repeated keywords

A bare name must be unique to read or write directly; `HISTORY`-style
keywords that repeat are built with
[`Header::append`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.append)
and read back with `get_all`/`count`, or an `("HISTORY", n)` key for one
occurrence.

```rust,no_run
# use xisf_header::Header;
# let mut header = Header::new();
header.append("HISTORY", "reduced with siril")?;
header.append("HISTORY", "stacked 20x300s")?;
# Ok::<(), xisf_header::Error>(())
```

## Attach XISF properties

XISF `<Property>` elements are a separate namespace from FITS keywords, keyed
by a colon-delimited id.
[`Header::set_property`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.set_property)
creates a `String`-typed property;
[`Header::set_property_with_type`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.set_property_with_type)
records an explicit XISF type (e.g. `Float32`), which round-trips verbatim.

```rust,no_run
# use xisf_header::Header;
# let mut header = Header::new();
header.set_property("Observation:Object:Name", "NGC 7000")?;
header.set_property_with_type("Instrument:Telescope:FocalLength", "0.53", "Float32")?;
# Ok::<(), xisf_header::Error>(())
```

## Serialize and assemble a new file

[`Header::to_header_bytes`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.to_header_bytes)
emits the preamble plus XML header from
[`StructuralHints`](https://docs.rs/xisf-header/latest/xisf_header/struct.StructuralHints.html),
with the `<Image location>` attachment offset already sized to fit data
matching those hints;
[`Header::parse`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.parse)
reads one back. Append your own pixel data to complete the container.

```rust,no_run
# use xisf_header::{Header, StructuralHints};
# let header = Header::new();
let hints = StructuralHints::default(); // 1x1x1 8-bit grayscale = 1 byte
let mut container = header.to_header_bytes(&hints);
container.push(0); // the caller's own pixel data
assert_eq!(Header::parse(&container)?, header);
# Ok::<(), xisf_header::Error>(())
```

## Round-trip through a file

Write the assembled container and read it back with
[`Header::read_from_file`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.read_from_file).

```rust,no_run
# use xisf_header::{Header, StructuralHints};
# let header = Header::new();
# let hints = StructuralHints::default();
let path = "master-dark.xisf";
let mut container = header.to_header_bytes(&hints);
container.push(0);
std::fs::write(path, &container)?;
let reloaded = Header::read_from_file(path)?;
assert_eq!(reloaded, header);
# Ok::<(), xisf_header::Error>(())
```

## Edit a file's header in place

[`Header::update_file`](https://docs.rs/xisf-header/latest/xisf_header/struct.Header.html#method.update_file)
reads a file's header, applies an edit closure, and splices the result back
into the file — byte-exact and data-preserving. Everything outside the
edited `<FITSKeyword>`/`<Property>` elements (unmodeled XML, whitespace, the
attached pixel data) survives untouched, and a no-op edit reproduces the
file byte-for-byte. If the edit changes the header's length, the `<Image
location>` offset is recomputed and the original data moves (unchanged) to
the new offset — `SIZE` never changes.

```rust,no_run
# use xisf_header::Header;
Header::update_file("master-dark.xisf", |h| {
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

## Handling errors

Keyword accessors return
[`Result`](https://docs.rs/xisf-header/latest/xisf_header/type.Result.html):
[`Error::Ambiguous`](https://docs.rs/xisf-header/latest/xisf_header/enum.Error.html)
signals a bare name that matches more than one keyword — select an occurrence
with an `(name, n)` key instead.

```rust,no_run
# use xisf_header::{Error, Header};
# let mut header = Header::new();
# header.append("HISTORY", "reduced with siril").unwrap();
# header.append("HISTORY", "stacked 20x300s").unwrap();
assert!(matches!(
    header.get_str("HISTORY"),
    Err(Error::Ambiguous { count: 2, .. })
));
assert_eq!(header.get_str(("HISTORY", 1))?, Some("stacked 20x300s"));
# Ok::<(), xisf_header::Error>(())
```
