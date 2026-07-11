//! Serialization: emit an XISF container (or just its header) from a [`Header`].

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
    /// Serialize into a complete, self-contained XISF container: the 16-byte
    /// preamble, the UTF-8 XML header (with an `<Image>` built from `hints`), and
    /// a zero-filled data block matching the hinted geometry.
    ///
    /// `Header::parse(&header.to_bytes(&hints))` round-trips back to `header`.
    #[must_use]
    pub fn to_bytes(&self, hints: &StructuralHints) -> Vec<u8> {
        self.build(hints, true)
    }

    /// Serialize just the header block — the preamble plus the XML header, with
    /// no data attached. The `<Image location>` points at the byte offset
    /// immediately after the header, where a caller doing in-place editing
    /// appends the image data itself.
    #[must_use]
    pub fn to_header_bytes(&self, hints: &StructuralHints) -> Vec<u8> {
        self.build(hints, false)
    }

    /// Write a complete XISF container to `path`.
    ///
    /// # Errors
    ///
    /// Propagates any I/O error from writing the file.
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P, hints: &StructuralHints) -> Result<()> {
        std::fs::write(path, self.to_bytes(hints))?;
        Ok(())
    }

    /// Read a file's header, apply `edit`, and write the container back.
    ///
    /// # Errors
    ///
    /// Propagates any error from [`Header::read_from_file`] or the write.
    pub fn update_file<P: AsRef<Path>>(
        path: P,
        hints: &StructuralHints,
        edit: impl FnOnce(&mut Self),
    ) -> Result<()> {
        let mut header = Self::read_from_file(&path)?;
        edit(&mut header);
        header.write_to_file(&path, hints)
    }

    /// Assemble the container. `with_data` appends the zero-filled data block.
    fn build(&self, hints: &StructuralHints, with_data: bool) -> Vec<u8> {
        let size = data_size(hints);

        // Two-pass render: the attachment offset depends on the header length,
        // which depends on the offset's text. A fixed-width offset keeps the
        // length identical between passes.
        let placeholder = "0".repeat(OFFSET_WIDTH);
        let xml_len = self.render_xml(hints, &placeholder, size).len();
        let offset = 16 + xml_len;
        let offset_str = format!("{offset:0width$}", width = OFFSET_WIDTH);
        let xml = self.render_xml(hints, &offset_str, size);
        debug_assert_eq!(xml.len(), xml_len, "offset width must not change length");

        let mut out = Vec::with_capacity(16 + xml.len() + if with_data { size } else { 0 });
        out.extend_from_slice(SIGNATURE);
        out.extend_from_slice(&u32::try_from(xml.len()).unwrap_or(u32::MAX).to_le_bytes());
        out.extend_from_slice(&[0u8; 4]); // reserved
        out.extend_from_slice(&xml);
        if with_data {
            out.resize(out.len() + size, 0);
        }
        out
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
            let value = match &kw.value {
                Value::Str(s) => format!("'{s}'"),
                Value::Literal(s) => s.clone(),
            };
            e.push_attribute(("value", value.as_str()));
            e.push_attribute(("comment", kw.comment.as_str()));
            w.write_event(Event::Empty(e)).expect(INFALLIBLE);
        }

        for (id, value) in &self.properties {
            let mut e = BytesStart::new("Property");
            e.push_attribute(("id", id.as_str()));
            e.push_attribute(("type", "String"));
            let value = format!("'{value}'");
            e.push_attribute(("value", value.as_str()));
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
