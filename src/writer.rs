//! Serialization: emit a self-contained XISF container from a [`Header`].

use std::path::Path;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::Writer;

use crate::error::Result;
use crate::header::Header;
use crate::reader::SIGNATURE;

/// Width of the zero-padded attachment offset embedded in the XML. Fixed so the
/// rendered header length is independent of the offset's magnitude.
const OFFSET_WIDTH: usize = 12;

/// Size, in bytes, of the placeholder attachment `to_bytes` appends.
const ATTACHMENT_SIZE: usize = 1;

impl Header {
    /// Serialize this header into a real, self-contained XISF container:
    /// the 16-byte preamble, the UTF-8 XML header, and a minimal placeholder
    /// attachment referenced by the `<Image location="attachment:…">`.
    ///
    /// `Header::parse(&header.to_bytes())` round-trips back to `header`.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        // Two-pass render: the attachment offset depends on the header length,
        // which depends on the offset's text. A fixed-width, zero-padded offset
        // keeps the length identical between passes.
        let placeholder = "0".repeat(OFFSET_WIDTH);
        let xml_len = self.render_xml(&placeholder).len();
        let offset = 16 + xml_len;
        let offset_str = format!("{offset:0width$}", width = OFFSET_WIDTH);
        let xml = self.render_xml(&offset_str);
        debug_assert_eq!(xml.len(), xml_len, "offset width must not change length");

        let mut out = Vec::with_capacity(16 + xml.len() + ATTACHMENT_SIZE);
        out.extend_from_slice(SIGNATURE);
        out.extend_from_slice(&(xml.len() as u32).to_le_bytes());
        out.extend_from_slice(&[0u8; 4]); // reserved
        out.extend_from_slice(&xml);
        out.extend_from_slice(&[0u8; ATTACHMENT_SIZE]); // placeholder attachment
        out
    }

    /// Write this header as a complete XISF container to `path`.
    ///
    /// # Errors
    ///
    /// Propagates any I/O error from writing the file.
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        std::fs::write(path, self.to_bytes())?;
        Ok(())
    }

    /// Read a file's header, apply `edit`, and write the container back.
    ///
    /// The rewritten file is header-only (with the placeholder attachment); it
    /// does not preserve the original file's pixel payload.
    ///
    /// # Errors
    ///
    /// Propagates any error from [`Header::read_from_file`] or
    /// [`Header::write_to_file`].
    pub fn update_file<P: AsRef<Path>>(path: P, edit: impl FnOnce(&mut Self)) -> Result<()> {
        let mut header = Self::read_from_file(&path)?;
        edit(&mut header);
        header.write_to_file(&path)
    }

    /// Render the XML header with a given attachment offset string.
    ///
    /// Writing to an in-memory `Vec` is infallible, so the `quick-xml`
    /// `io::Result`s are unwrapped.
    fn render_xml(&self, offset_str: &str) -> Vec<u8> {
        const INFALLIBLE: &str = "writing XML to an in-memory buffer cannot fail";

        let mut w = Writer::new(Vec::new());
        w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .expect(INFALLIBLE);

        let mut xisf = BytesStart::new("xisf");
        xisf.push_attribute(("version", "1.0"));
        xisf.push_attribute(("xmlns", "http://www.pixinsight.com/xisf"));
        w.write_event(Event::Start(xisf)).expect(INFALLIBLE);

        let mut image = BytesStart::new("Image");
        image.push_attribute(("geometry", "1:1:1"));
        image.push_attribute(("sampleFormat", "UInt8"));
        let location = format!("attachment:{offset_str}:{ATTACHMENT_SIZE}");
        image.push_attribute(("location", location.as_str()));
        w.write_event(Event::Start(image)).expect(INFALLIBLE);

        for kw in &self.keywords {
            let mut e = BytesStart::new("FITSKeyword");
            e.push_attribute(("name", kw.name.as_str()));
            let value = format!("'{}'", kw.value);
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
