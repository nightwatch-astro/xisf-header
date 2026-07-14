// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Serialization: emit an XISF container (or just its header) from a [`Header`].

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::Writer;

use crate::error::Result;
use crate::header::{Header, StructuralHints};
use crate::reader::SIGNATURE;
use crate::value::Value;

/// Fixed width of the zero-padded attachment offset, so the rendered header
/// length is independent of the offset's magnitude.
const OFFSET_WIDTH: usize = 12;

impl Header {
    /// Serialize the header block — the 16-byte preamble plus the UTF-8 XML
    /// header, with no data attached. The `<Image location>` points at the
    /// byte offset immediately after the header, sized per `hints`, where a
    /// caller assembling a new file appends the image data itself.
    ///
    /// `Header::parse(&header.to_header_bytes(&hints))` round-trips back to
    /// `header`.
    ///
    /// ```
    /// use xisf_header::{Header, StructuralHints};
    ///
    /// let mut header = Header::new();
    /// header.set("IMAGETYP", "Master Dark").unwrap();
    /// let hints = StructuralHints::default();
    ///
    /// let header_only = header.to_header_bytes(&hints);
    /// assert_eq!(Header::parse(&header_only).unwrap(), header);
    /// ```
    #[must_use]
    pub fn to_header_bytes(&self, hints: &StructuralHints) -> Vec<u8> {
        self.render_container_header(hints, data_size(hints))
    }

    /// Write a complete XISF container to a **new** file: the preamble + XML
    /// header (with the `<Image>` element from `hints`, and a `location`
    /// attachment `SIZE` equal to `data.len()`) followed by `data` verbatim.
    ///
    /// `data` is the caller's own pixel bytes — this crate never fabricates
    /// image data. Pass `&[]` for a header-only container (`SIZE` 0).
    ///
    /// This creates a **new file only**: it fails with
    /// [`Error::Io`](crate::Error::Io) (`ErrorKind::AlreadyExists`) if `path`
    /// already exists, rather than overwriting it. To edit an existing file's
    /// header in place, use [`update_file`](Self::update_file) instead.
    ///
    /// # Errors
    ///
    /// Propagates any I/O error opening or writing `path`, including
    /// `AlreadyExists` when the path already exists.
    ///
    /// ```
    /// use xisf_header::{Header, StructuralHints};
    ///
    /// let path = std::env::temp_dir().join("xisf-header-doctest-write.xisf");
    /// # std::fs::remove_file(&path).ok();
    /// let mut header = Header::new();
    /// header.set("IMAGETYP", "Master Dark")?;
    /// let hints = StructuralHints::default();
    /// let data = [0u8; 4]; // the caller's own pixel bytes
    ///
    /// header.write_to_file(&path, &hints, &data)?;
    ///
    /// let bytes = std::fs::read(&path)?;
    /// assert_eq!(&bytes[bytes.len() - 4..], &data);
    /// let reloaded = Header::read_from_file(&path)?;
    /// assert_eq!(reloaded, header);
    ///
    /// // A second write to the same path never clobbers it.
    /// assert!(header.write_to_file(&path, &hints, &data).is_err());
    /// # std::fs::remove_file(&path).ok();
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn write_to_file<P: AsRef<Path>>(
        &self,
        path: P,
        hints: &StructuralHints,
        data: &[u8],
    ) -> Result<()> {
        let header_bytes = self.render_container_header(hints, data.len());
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&header_bytes)?;
        file.write_all(data)?;
        Ok(())
    }

    /// Render the preamble + XML header for a container whose data block is
    /// `size` bytes, per `hints`. Shared by [`to_header_bytes`](Self::to_header_bytes)
    /// (`size` derived from `hints`' geometry) and
    /// [`write_to_file`](Self::write_to_file) (`size` = the caller's actual
    /// `data.len()`, so the emitted `SIZE` always matches the bytes on disk).
    fn render_container_header(&self, hints: &StructuralHints, size: usize) -> Vec<u8> {
        // Two-pass render: the attachment offset depends on the header length,
        // which depends on the offset's text. A fixed-width offset keeps the
        // length identical between passes.
        let placeholder = "0".repeat(OFFSET_WIDTH);
        let xml_len = self.render_xml(hints, &placeholder, size).len();
        let offset = 16 + xml_len;
        let offset_str = format!("{offset:0width$}", width = OFFSET_WIDTH);
        let xml = self.render_xml(hints, &offset_str, size);
        debug_assert_eq!(xml.len(), xml_len, "offset width must not change length");

        let mut out = Vec::with_capacity(16 + xml.len());
        out.extend_from_slice(SIGNATURE);
        out.extend_from_slice(&u32::try_from(xml.len()).unwrap_or(u32::MAX).to_le_bytes());
        out.extend_from_slice(&[0u8; 4]); // reserved
        out.extend_from_slice(&xml);
        out
    }

    /// Read a file's header, apply `edit`, and splice the result back into
    /// the file in place: byte-exact and data-preserving. Every byte outside
    /// the edited `<FITSKeyword>`/`<Property>` elements — unmodeled XML
    /// (`Metadata`, `Resolution`, thumbnails, …), whitespace, and the
    /// attached data block — survives untouched. A no-op edit reproduces the
    /// input file byte-for-byte. If the header's XML length changes, the
    /// `<Image location="attachment:OFFSET:SIZE">` offset is recomputed and
    /// the original data bytes are moved (unchanged) to the new offset;
    /// `SIZE` never changes.
    ///
    /// This requires the common single-image layout: exactly one `<Image
    /// location="attachment:…">` element. A file with zero or multiple
    /// attachments (e.g. a `Thumbnail` alongside the `Image`), or whose edit
    /// needs to add elements to a self-closing `<Image/>`, is rejected with
    /// [`Error::Unsupported`](crate::Error::Unsupported) rather than risking
    /// data loss.
    ///
    /// The write is atomic — a sibling temp file is renamed over the target
    /// — and follows symlinks (a symlinked `path` stays a symlink to the same
    /// target) and preserves the target's unix permission mode.
    ///
    /// # Errors
    ///
    /// Propagates any error from reading or re-parsing the file, from
    /// `edit`, or [`Error::Unsupported`](crate::Error::Unsupported) for a
    /// layout the splice can't safely target. On error the file is left
    /// untouched.
    ///
    /// ```
    /// use xisf_header::{Header, StructuralHints};
    ///
    /// let path = std::env::temp_dir().join("xisf-header-doctest-update.xisf");
    /// let mut header = Header::new();
    /// header.set("IMAGETYP", "Master Dark")?;
    /// let hints = StructuralHints::default(); // 1x1x1 UInt8 = 1 byte of data
    /// let mut container = header.to_header_bytes(&hints);
    /// container.push(0xAB); // the caller's own pixel data
    /// std::fs::write(&path, &container)?;
    ///
    /// Header::update_file(&path, |h| {
    ///     h.set("OBJECT", "NGC 7000")?;
    ///     Ok(())
    /// })?;
    ///
    /// assert_eq!(std::fs::read(&path)?.last(), Some(&0xAB)); // pixel data preserved
    /// let edited = Header::read_from_file(&path)?;
    /// assert_eq!(edited.get_str("OBJECT")?, Some("NGC 7000"));
    /// # std::fs::remove_file(&path).ok();
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn update_file<P: AsRef<Path>>(
        path: P,
        edit: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        crate::splice::update_file(path, edit)
    }

    /// Render the XML header. Writing to an in-memory `Vec` is infallible.
    fn render_xml(&self, hints: &StructuralHints, offset_str: &str, size: usize) -> Vec<u8> {
        const INFALLIBLE: &str = "writing XML to an in-memory buffer cannot fail";

        let mut w = Writer::new(Vec::new());
        w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .expect(INFALLIBLE);

        let mut xisf = BytesStart::new("xisf");
        xisf.push_attribute(("version", "1.0"));
        xisf.push_attribute(("xmlns", "http://www.pixinsight.com/xisf"));
        w.write_event(Event::Start(xisf)).expect(INFALLIBLE);

        let mut image = BytesStart::new("Image");
        image.push_attribute(("geometry", hints.geometry.as_str()));
        image.push_attribute(("sampleFormat", hints.sample_format.as_str()));
        image.push_attribute(("colorSpace", hints.color_space.as_str()));
        let location = format!("attachment:{offset_str}:{size}");
        image.push_attribute(("location", location.as_str()));
        w.write_event(Event::Start(image)).expect(INFALLIBLE);

        for kw in &self.keywords {
            let mut e = BytesStart::new("FITSKeyword");
            e.push_attribute(("name", kw.name.as_str()));
            if crate::keyword::is_commentary(&kw.name) {
                // FITS commentary keywords have no value: the free text lives
                // in `comment`, with an empty `value` (see `is_commentary`).
                e.push_attribute(("value", ""));
                e.push_attribute(("comment", kw.value.text()));
            } else {
                let value = match &kw.value {
                    Value::Str(s) => format!("'{s}'"),
                    Value::Literal(s) => s.clone(),
                };
                e.push_attribute(("value", value.as_str()));
                e.push_attribute(("comment", kw.comment.as_str()));
            }
            w.write_event(Event::Empty(e)).expect(INFALLIBLE);
        }

        for (id, p) in &self.properties {
            let mut e = BytesStart::new("Property");
            e.push_attribute(("id", id.as_str()));
            e.push_attribute(("type", p.type_.as_str()));
            e.push_attribute(("value", p.value.as_str()));
            if !p.format.is_empty() {
                e.push_attribute(("format", p.format.as_str()));
            }
            if !p.comment.is_empty() {
                e.push_attribute(("comment", p.comment.as_str()));
            }
            w.write_event(Event::Empty(e)).expect(INFALLIBLE);
        }

        w.write_event(Event::End(BytesEnd::new("Image")))
            .expect(INFALLIBLE);
        w.write_event(Event::End(BytesEnd::new("xisf")))
            .expect(INFALLIBLE);

        w.into_inner()
    }
}

/// Byte size of the data block implied by the hinted geometry and sample format.
fn data_size(hints: &StructuralHints) -> usize {
    let samples: Option<usize> = hints
        .geometry
        .split(':')
        .map(|d| d.trim().parse::<usize>().ok())
        .collect::<Option<Vec<_>>>()
        .map(|dims| dims.iter().product());
    let samples = samples.filter(|&s| s > 0).unwrap_or(1);
    samples
        .saturating_mul(bytes_per_sample(&hints.sample_format))
        .max(1)
}

/// Bytes per sample for an XISF `sampleFormat`.
fn bytes_per_sample(format: &str) -> usize {
    match format {
        "UInt16" | "Int16" => 2,
        "UInt32" | "Int32" | "Float32" => 4,
        "UInt64" | "Int64" | "Float64" | "Complex32" => 8,
        "Complex64" => 16,
        _ => 1, // UInt8/Int8 and anything unrecognized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hints(geometry: &str, sample_format: &str) -> StructuralHints {
        StructuralHints {
            geometry: geometry.to_owned(),
            sample_format: sample_format.to_owned(),
            color_space: "Gray".to_owned(),
        }
    }

    #[test]
    fn bytes_per_sample_matrix() {
        for (format, bytes) in [
            ("UInt8", 1),
            ("Int8", 1),
            ("UInt16", 2),
            ("Int16", 2),
            ("UInt32", 4),
            ("Int32", 4),
            ("Float32", 4),
            ("UInt64", 8),
            ("Int64", 8),
            ("Float64", 8),
            ("Complex32", 8),
            ("Complex64", 16),
            ("SomethingElse", 1),
        ] {
            assert_eq!(bytes_per_sample(format), bytes, "{format}");
        }
    }

    #[test]
    fn data_size_from_geometry() {
        assert_eq!(data_size(&hints("1:1:1", "UInt8")), 1);
        assert_eq!(data_size(&hints("100:100:3", "Float32")), 120_000);
        assert_eq!(data_size(&hints("16:16:1", "UInt16")), 512);
        // Malformed or zero geometry falls back to a single sample.
        assert_eq!(data_size(&hints("abc", "UInt8")), 1);
        assert_eq!(data_size(&hints("0:0:0", "Float32")), 4);
        assert_eq!(data_size(&hints("", "UInt8")), 1);
    }

    #[test]
    fn header_only_output_has_no_data_block() {
        let h = Header::new();
        let hints = StructuralHints::default();
        let bytes = h.to_header_bytes(&hints);
        let xml_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        assert_eq!(bytes.len(), 16 + xml_len);
    }

    #[test]
    fn attachment_offset_is_fixed_width_and_correct() {
        let h = Header::new();
        let bytes = h.to_header_bytes(&StructuralHints::default());
        let xml_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let xml = std::str::from_utf8(&bytes[16..16 + xml_len]).unwrap();
        let offset = format!("{:0width$}", 16 + xml_len, width = OFFSET_WIDTH);
        assert!(
            xml.contains(&format!("attachment:{offset}:")),
            "location must point right past the header: {xml}"
        );
    }

    #[test]
    fn xml_special_characters_round_trip() {
        let mut h = Header::new();
        h.set("OBJECT", "a<b&\"c'd").unwrap();
        h.set_comment("OBJECT", "less < & \"quoted\"").unwrap();
        h.set_property("Notes:Text", "x<y&z").unwrap();
        let parsed = Header::parse(&h.to_header_bytes(&StructuralHints::default())).unwrap();
        assert_eq!(parsed, h);
        assert_eq!(parsed.get_str("OBJECT").unwrap(), Some("a<b&\"c'd"));
        assert_eq!(parsed.property("Notes:Text"), Some("x<y&z"));
    }

    #[test]
    fn empty_header_round_trips() {
        let h = Header::new();
        let parsed = Header::parse(&h.to_header_bytes(&StructuralHints::default())).unwrap();
        assert_eq!(parsed, h);
    }

    /// HISTORY/COMMENT have no FITS value — the text must land in the
    /// `comment` attribute with an empty `value`, not a quoted `value` (the
    /// bug this fix corrects: the old form was malformed FITS commentary).
    #[test]
    fn commentary_keywords_serialize_as_empty_value_with_comment_text() {
        let mut h = Header::new();
        h.append("HISTORY", "reduced with siril").unwrap();
        h.append("COMMENT", "processed in PixInsight").unwrap();
        let bytes = h.to_header_bytes(&StructuralHints::default());
        let xml = std::str::from_utf8(&bytes[16..]).unwrap();

        assert!(
            xml.contains(r#"name="HISTORY" value="" comment="reduced with siril""#),
            "xml: {xml}"
        );
        assert!(
            xml.contains(r#"name="COMMENT" value="" comment="processed in PixInsight""#),
            "xml: {xml}"
        );
        assert!(!xml.contains("&apos;reduced with siril&apos;"));
        assert!(!xml.contains("&apos;processed in PixInsight&apos;"));
    }

    /// Non-commentary keywords keep their pre-fix serialization: strings
    /// quoted in `value`, numbers bare, `comment` used for the FITS comment.
    #[test]
    fn non_commentary_keywords_are_unaffected() {
        let mut h = Header::new();
        h.set("OBJECT", "M31").unwrap();
        h.set("GAIN", 100_i64).unwrap();
        let bytes = h.to_header_bytes(&StructuralHints::default());
        let xml = std::str::from_utf8(&bytes[16..]).unwrap();

        assert!(
            xml.contains(r#"name="OBJECT" value="&apos;M31&apos;""#),
            "{xml}"
        );
        assert!(xml.contains(r#"name="GAIN" value="100""#), "{xml}");
    }

    /// `write_to_file`'s emitted `SIZE` must track the caller's actual
    /// `data.len()`, not the size implied by `hints`' geometry — the two can
    /// legitimately diverge (e.g. a caller passing header-only `&[]` against
    /// non-trivial hints).
    #[test]
    fn write_to_file_size_matches_data_len_not_hints() {
        let h = Header::new();
        let mismatched_hints = hints("4:4:1", "UInt16"); // implies 32 bytes
        let data = [1u8, 2, 3]; // actual payload is 3 bytes
        let path = std::env::temp_dir().join(format!(
            "xisf-header-writer-size-{}-{}.xisf",
            std::process::id(),
            line!()
        ));
        std::fs::remove_file(&path).ok();

        h.write_to_file(&path, &mismatched_hints, &data).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let xml_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let xml = std::str::from_utf8(&bytes[16..16 + xml_len]).unwrap();
        assert!(
            xml.contains(&format!(":{}\"", data.len())),
            "SIZE must equal data.len() (3), not the hints-implied 32: {xml}"
        );
        assert_eq!(bytes.len(), 16 + xml_len + data.len());
        assert_eq!(&bytes[bytes.len() - data.len()..], &data);

        std::fs::remove_file(&path).ok();
    }

    /// `write_to_file` must never clobber an existing file — the crate's only
    /// path for editing an existing file is `update_file`.
    #[test]
    fn write_to_file_errors_if_path_exists() {
        let h = Header::new();
        let path = std::env::temp_dir().join(format!(
            "xisf-header-writer-exists-{}-{}.xisf",
            std::process::id(),
            line!()
        ));
        std::fs::write(&path, b"pre-existing content").unwrap();

        let err = h
            .write_to_file(&path, &StructuralHints::default(), &[])
            .unwrap_err();
        assert!(matches!(
            err,
            crate::Error::Io(e) if e.kind() == std::io::ErrorKind::AlreadyExists
        ));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"pre-existing content",
            "the existing file must be left untouched"
        );

        std::fs::remove_file(&path).ok();
    }
}
