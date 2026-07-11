# Spec 001 — xisf-header crate

Status: implemented · Mode: lightweight

## Goal

A standalone, publishable Rust crate that reads and writes a generic XISF header
container. It extracts the FITS keywords embedded in an XISF file and supports
full CRUD over those keywords, producing a `Header` value that can be written
back into a real XISF container.

## Functional requirements

- **FR-1 Parse.** `Header::parse(&[u8]) -> Result<Header>` validates the 16-byte
  preamble: bytes 0–7 == `XISF0100` (else `InvalidData`), bytes 8–11 = XML length
  `u32` LE (capped at 8 MiB → `HeaderTooLarge`), bytes 12–15 reserved (ignored on
  read). The XML header must be valid UTF-8. Header-only — never read pixel data.
- **FR-2 Extract keywords.** Read `<FITSKeyword name= value= comment=>` elements.
  Attribute names are case-insensitive; the `name` value is kept verbatim. A value
  wrapped in FITS single quotes has exactly one `'…'` layer stripped.
- **FR-3 Extract properties.** Read `<Property id= value=>` elements into a
  `Map<String, String>`.
- **FR-4 CRUD.** Create/read/update/delete keywords, single and in bulk:
  `set`/`push`/`extend`, `get`/`get_all`/typed getters, `remove`/`remove_all`.
  Properties have `property`/`set_property`/`remove_property`.
- **FR-5 Write.** `Header::to_bytes() -> Vec<u8>` emits a real container:
  `XISF0100` + `u32` LE XML length + 4 reserved zero bytes + UTF-8 XML with an
  `<Image>` whose `location="attachment:<offset>:<size>"` points at a tiny
  attachment written at `offset ≥ 16 + headerLen`. Strings are single-quote-wrapped
  inside `value=` (as NINA/PixInsight write them).
- **FR-6 File I/O.** `read_from_file` reads only the preamble + XML (never pixel
  data); `write_to_file` and `update_file` write a container.

## Acceptance

- Bad signature → error.
- `Header::parse(header.to_bytes()) == header` (round-trip).
- The written container passes its own signature/length validation.
- CRUD works for single and multiple keywords; typed getters parse values.

## Non-goals

- Reading, writing, or preserving pixel/attachment payloads.
- The `metadata_xisf` adapter mapping to `RawFileMetadata` (that lives in the
  PlateVault monorepo, which consumes this crate).
