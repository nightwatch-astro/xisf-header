# AGENTS.md

## Project description

`xisf-header` — read and write XISF image-file headers: extract FITS keywords and
CRUD the XISF header container. A publishable Rust crate that is
**header-only** — it never reads or writes pixel data.

XISF (Extensible Image Serialization Format) files begin with a 16-byte binary
preamble followed by a UTF-8 XML header. This crate parses that header into a
generic `Header` value (a list of `FitsKeyword`s plus a map of `<Property>`
elements), lets callers create/read/update/delete keywords and properties, and
serializes a `Header` back into a valid, self-contained XISF container.

## Layout

```
src/
  lib.rs      crate root, docs, public re-exports (incl. `pub use time`)
  error.rs    Error enum + Result alias
  key.rs      Key: unified "NAME" / ("NAME", n) keyword address
  value.rs    Value + FromField (read) + IntoValue (write) + Literal/Fixed/Sci
  keyword.rs  FitsKeyword record
  header.rs   Header: strict CRUD, typed get/set, atomic batch, property CRUD, StructuralHints
  reader.rs   Header::parse / read_from_file (preamble validation + XML extraction, incl. byte-span index)
  writer.rs   Header::to_header_bytes / update_file (delegates the splice to splice.rs)
  splice.rs   byte-exact update_file: diff + splice only edited elements, preserving unmodeled XML + data
tests/
  roundtrip.rs        integration tests (signature, round-trip, strict access, file I/O, CRUD)
  integration_xisf.rs byte-exact update_file against real XISF files (unmodeled XML + real data block)
specs/
  001-xisf-header/spec.md   spec of the read/edit/write requirements
docs/
  decisions/   architecture decision records
```

## Commands

- Build: `just build` (or `cargo build --all-targets`)
- Test: `just test` (or `cargo test`)
- Lint: `just lint` (clippy `-D warnings` + `cargo fmt --check`)
- Format: `just fmt`
- Publish check: `just publish-check` (`cargo publish --dry-run`)

## Conventions

- `#![forbid(unsafe_code)]`; public items are documented (`missing_docs = warn`).
- Reach for a good library instead of hand-rolling: `thiserror` for errors,
  `quick-xml` for XML, `time` for date/time, optional `serde` for
  (de)serialization. Prefer mature, pure-Rust crates (no C/sys — MSVC-safe).
- Keyword access is strict: a bare name must be unique or accessors return
  `Error::Ambiguous`; repeats are addressed with an `(name, n)` key.
- Header-only in what it *models*: keyword/property values, never pixel
  data. `update_file` does move the attached data block's raw bytes (to keep
  a file byte-exact after an edit), but never interprets or decodes them.
- Keep it simple and idiomatic; small, focused modules.

## MSRV

Rust 1.82.0 (declared in `Cargo.toml`, exercised by the CI `msrv` job).
