# 1. Header-editor architecture: alignment with fits-header

Status: Accepted (shared architecture as the reference for both crates).
Migration of xisf-header's implementation to the full model is an open decision — see *Open decision*.

## Context

`xisf-header` and `fits-header` are separate, single-format crates (they share no
code — see the separate-crates decision) that solve the same problem: parse a
file's header, CRUD its keywords/properties, and serialize it back. The
`fits-header` effort produced a complete architecture for a *faithful, byte-exact
header editor*, intended as the shared shape for both crates. This record captures
that shared architecture, where `xisf-header` diverges, its current position, and
the decisions each side owns.

## Shared architecture (both crates)

- **Faithful editor, not a normalizer.** Parse preserves every record in order.
  Byte-exact/verbatim round-trip: untouched records re-emit identical bytes; only
  created or modified records are re-serialized.
- **Storage.** Ordered list of physical records + a dirty bit; record kinds are
  internal; the public surface is accessor-first. `PartialEq` is semantic
  (name/value/comment); byte-equality is a test-only check.
- **Strict access, unified key.** `"NAME"` (unique-or-`Err(Ambiguous)`) or
  `("NAME", n)` for the n-th occurrence. Reads: `get::<T>(key) -> Result<Option<T>>`
  — `Err` only on a duplicate name, `Ok(None)` for absent/unparseable, never
  panics. Plus `get_all`, `count`, `cards()/iter()`.
- **Writes.** `set`/`append`/`remove` return `Result`; batches are atomic. `set`
  preserves an existing comment; `set_comment` edits it.
- **Types.** Reads via an open `FromField` trait; writes via `impl IntoValue`
  (`Literal`/`Fixed`/`Sci` wrappers); default f64 = shortest round-trip, normalized
  to read as a float.
- **Two outputs.** `to_bytes(&StructuralHints)` (complete object, synthesize
  mandatory metadata only-when-absent) and `to_header_bytes()` (header-only, for
  in-place editing: concatenate with the file's original data).
- **Deps/policy.** Pure-Rust, MSVC-safe: `time` (public, re-exported),
  `thiserror`, optional off-by-default `serde`, `proptest` (dev);
  `#![forbid(unsafe_code)]`, Apache-2.0, edition 2021.

## Where xisf-header diverges

- **Serialization mechanics.** XML via a pure-Rust reader/writer (`quick-xml`),
  not 80-byte cards; no CONTINUE (long strings are just long); the structural
  analogue is `<Image geometry= sampleFormat= colorSpace=>`.
- **Identifiers.** `<FITSKeyword>` names follow FITS keyword rules; `<Property>`
  ids follow XISF's namespaced-identifier syntax (not the FITS charset).
- **Native typing.** XISF `<Property>` types (Int8..64, Float32/64, String,
  TimePoint…) map straight onto `FromField`/`IntoValue`; `<FITSKeyword>` values
  stay untyped text like FITS.

## Current state of xisf-header

- 0.1.0 shipped/published on the initial semantic model (`Option` reads,
  first-occurrence-wins, a single `to_bytes`, concrete getters).
- 0.2 implemented on branch `feat/faithful-editor-0.2`: strict unified `Key`
  (`"NAME"` / `("NAME", n)`), `Result` reads that `Err` on duplicates, the open
  `FromField` read trait + `impl IntoValue` writes (`Literal`/`Fixed`/`Sci`), two
  outputs (`to_bytes`/`to_header_bytes`), atomic batch mutations, and `time` as a
  public dependency. Not yet byte-exact.

## Migration status

The strict-key + `FromField`/`IntoValue` + two-output surface is **done** in 0.2
(branch `feat/faithful-editor-0.2`), awaiting review/merge and a `crates.io`
release.

**Deferred:** byte-exact XML retention (retain each element's original source
span; dirty-bit re-emit). Its value depends on whether the consumer (PlateVault →
`RawFileMetadata`) needs verbatim byte preservation rather than the semantic
correctness the current model already delivers. Revisit when a concrete
verbatim-preservation requirement exists.

## Recommendations issued to the fits-header agent

1. Pull the RA/Dec + MJD↔calendar helpers out of core (or feature-gate them) —
   they are astronomy domain math, not header I/O.
2. Resolve byte-exact vs CONTINUE explicitly: reassemble on read, re-emit original
   bytes when the logical value is untouched, re-split only when edited; state the
   byte-exact guarantee as *per logical value* for long strings.
3. Reconcile the strict keyword charset (`A–Z 0–9 - _`, ≤8) with the vendor-key
   escape hatch; make an explicit lowercase/vendor decision.
4. Pin `to_bytes`: define the mandatory set (`SIMPLE`, `BITPIX`, `NAXIS`,
   `NAXIS1…NAXISn`), synthesize each missing one in FITS order, and decide the
   declared-geometry-without-data case (error vs zero-fill); route real-file edits
   through `to_header_bytes` + original data.

Minor: justify or generalize `u32` in the type set; tighten the reads-error wording
to "`Err` only on ambiguity, `Ok(None)` otherwise, never panic".

## Consequences

- The two crates stay separate — alignment is by shared design, not a shared code
  crate or a dependency link.
- `xisf-header` remains on its shipped 0.1 model until the migration decision is
  made; this record is the reference for that decision.
