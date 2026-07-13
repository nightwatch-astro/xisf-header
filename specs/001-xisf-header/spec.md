# Spec 001 — xisf-header crate

Status: implemented · Mode: lightweight

## Goal

A standalone, publishable Rust crate that reads, edits, and writes a generic XISF
header. It extracts the FITS keywords and `<Property>` elements embedded in an
XISF file, supports strict keyword-oriented CRUD with typed values, and serializes
a `Header` back into an XISF container. Header-only — it never touches pixel data.

## Functional requirements

- **FR-1 Parse.** `Header::parse(&[u8]) -> Result<Header>` validates the 16-byte
  preamble: bytes 0–7 == `XISF0100` (else `InvalidSignature`), bytes 8–11 = XML
  length `u32` LE (capped at 8 MiB → `HeaderTooLarge`), bytes 12–15 reserved
  (ignored on read). The XML header must be valid UTF-8.
- **FR-2 Extract keywords.** Read `<FITSKeyword name= value= comment=>` elements.
  Attribute names are case-insensitive; `name` is kept verbatim. A `value` wrapped
  in single quotes is a string value (one `'…'` layer stripped); anything else is a
  bare literal. The kind is preserved so a value round-trips as its own kind.
- **FR-3 Extract properties.** Read `<Property id= value=>` elements into a
  `Map<String, String>`.
- **FR-4 Strict keyword access.** Keywords form an ordered list. A `Key` is either
  `"NAME"` (must be unique) or `("NAME", n)` (the n-th occurrence). `get::<T>`,
  `set`, and `remove` on a bare name return `Ambiguous` when it repeats;
  `get_all`/`count` and the `(name, n)` key address repeats. `append` always adds.
  Batch `set_many`/`remove_many` are atomic (validate all, then apply all-or-none).
- **FR-5 Typed values.** Reads go through the open `FromField` trait
  (`String`, `f64`, `i64`, `u32`, `bool`, `time::PrimitiveDateTime`), with
  `get_str`/`get_f64`/… wrappers; `i64`/`u32` accept a decimal form (`20.0` → `20`).
  Writes take `impl IntoValue`: the Rust type selects string vs. bare-literal
  formatting, with `Literal`/`Fixed`/`Sci` for controlled output and default `f64`
  as the shortest round-trippable float.
- **FR-6 Write a new header block.** `to_header_bytes(&StructuralHints)` emits
  the header block alone (`XISF0100` + `u32` LE XML length + 4 reserved bytes +
  UTF-8 XML with an `<Image>` built from the hints, its `location` offset sized
  for hint-sized data). Callers append their own data to assemble a container.
  (Superseded: the earlier `to_bytes(&StructuralHints)` — which fabricated a
  zero-filled data block — was removed; it could not preserve real pixel data.)
- **FR-7 File I/O.** `read_from_file` reads only the preamble + XML (never pixel
  data). `update_file(path, edit)` edits a file's header in place, byte-exact:
  it splices only the changed `<FITSKeyword>`/`<Property>` elements into the
  original bytes, preserves unmodeled XML and the attached data block verbatim
  (a no-op edit reproduces the file byte-for-byte), and recomputes the `<Image
  location>` offset when the header length changes. It takes no `StructuralHints`.
  Layouts it cannot splice safely (zero or multiple attachments) return
  `Unsupported`. (Superseded: the earlier `write_to_file`, which rewrote a
  container from `StructuralHints` and zero-filled data, was removed.)

## Acceptance

- Bad signature → error; truncated input → error.
- `Header::parse(header.to_header_bytes(&hints)) == header` (round-trip),
  including value kind (string vs. literal) and comments.
- A no-op `update_file` reproduces the input file byte-for-byte; an edit changes
  only the intended keyword/property (plus the offset) while unmodeled XML and
  the attached data survive intact.
- A bare-name `get`/`set`/`remove` on a repeated keyword returns `Ambiguous`;
  `(name, n)` and `get_all`/`count` address the repeats.
- Batch mutations are atomic; typed getters read values; invalid names are rejected.

## Non-goals

- Interpreting, decoding, or synthesizing pixel/attachment payloads. (`update_file`
  moves the attached data block's raw bytes to keep a file byte-exact after an
  edit, but never reads or reconstructs their content.)
- The `metadata_xisf` adapter mapping to `RawFileMetadata` (that lives in the
  PlateVault monorepo, which consumes this crate).
