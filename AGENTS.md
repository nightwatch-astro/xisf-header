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
  lib.rs      crate root, docs, public re-exports, file helpers
  error.rs    Error enum + Result alias
  keyword.rs  FitsKeyword struct + typed value accessors
  header.rs   Header struct: CRUD (single + bulk) + typed getters + property CRUD
  reader.rs   parse(&[u8]) -> Header  (preamble validation + XML extraction)
  writer.rs   Header::to_bytes() -> Vec<u8>  (emits a real XISF container)
tests/
  roundtrip.rs   integration tests (signature, round-trip, file I/O, CRUD)
specs/
  001-xisf-header/spec.md   lightweight spec of the extraction requirements
```

## Commands

- Build: `just build` (or `cargo build --all-targets`)
- Test: `just test` (or `cargo test`)
- Lint: `just lint` (clippy `-D warnings` + `cargo fmt --check`)
- Format: `just fmt`
- Publish check: `just publish-check` (`cargo publish --dry-run`)

## Conventions

- `#![forbid(unsafe_code)]`; public items are documented (`missing_docs = warn`).
- Reach for a good library instead of hand-rolling: `thiserror` for the error
  type, `quick-xml` for XML, optional `serde` for (de)serialization. Prefer
  mature, pure-Rust crates (no C/sys — keeps the build MSVC-safe).
- Header-only: never read or write pixel/attachment payloads beyond the tiny
  placeholder attachment `to_bytes()` writes to make a container self-contained.
- Keep it simple and idiomatic; small, focused modules.

## MSRV

Rust 1.82.0 (declared in `Cargo.toml`, exercised by the CI `msrv` job).
