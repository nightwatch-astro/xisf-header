//! Parsing: preamble validation and XML extraction into a [`Header`].

use std::fs::File;
use std::io::Read;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::{Error, Result};
use crate::header::Header;
use crate::keyword::FitsKeyword;
use crate::property::Property;
use crate::value::Value;

/// The 8-byte XISF monolithic-file signature.
pub(crate) const SIGNATURE: &[u8; 8] = b"XISF0100";

/// Upper bound on the declared XML-header length (8 MiB).
pub(crate) const MAX_HEADER_LEN: usize = 8 * 1024 * 1024;

impl Header {
    /// Parse an XISF header from raw container bytes.
    ///
    /// Validates the 16-byte preamble — bytes 0–7 are the `XISF0100`
    /// signature, bytes 8–11 are the little-endian XML-header length (capped at
    /// 8 MiB), bytes 12–15 are reserved and ignored — then decodes the UTF-8 XML
    /// header and extracts every `<FITSKeyword>` and `<Property>`. Pixel/
    /// attachment data beyond the header is never read.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TooSmall`] if the input is truncated,
    /// [`Error::InvalidSignature`] on a bad signature, [`Error::HeaderTooLarge`]
    /// if the declared header exceeds the cap, or an XML/UTF-8 error if the
    /// header itself is malformed.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 16 {
            return Err(Error::TooSmall {
                needed: 16,
                got: bytes.len(),
            });
        }
        if &bytes[0..8] != SIGNATURE {
            return Err(Error::InvalidSignature);
        }
        let xml_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        // bytes[12..16] are reserved and ignored on read.
        if xml_len > MAX_HEADER_LEN {
            return Err(Error::HeaderTooLarge {
                len: xml_len,
                max: MAX_HEADER_LEN,
            });
        }
        let end = 16 + xml_len;
        if bytes.len() < end {
            return Err(Error::TooSmall {
                needed: end,
                got: bytes.len(),
            });
        }
        let xml = std::str::from_utf8(&bytes[16..end])?;
        parse_xml(xml)
    }

    /// Read and parse the header of an XISF file, reading only the preamble and
    /// XML header — never the pixel/attachment payload.
    ///
    /// # Errors
    ///
    /// Propagates I/O errors and any [`Header::parse`] error.
    pub fn read_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;

        let mut preamble = [0u8; 16];
        file.read_exact(&mut preamble)?;
        if &preamble[0..8] != SIGNATURE {
            return Err(Error::InvalidSignature);
        }
        let xml_len =
            u32::from_le_bytes([preamble[8], preamble[9], preamble[10], preamble[11]]) as usize;
        if xml_len > MAX_HEADER_LEN {
            return Err(Error::HeaderTooLarge {
                len: xml_len,
                max: MAX_HEADER_LEN,
            });
        }

        let mut buf = vec![0u8; 16 + xml_len];
        buf[..16].copy_from_slice(&preamble);
        file.read_exact(&mut buf[16..])?;
        Self::parse(&buf)
    }
}

/// A `<Property>` opened as a start tag, which may carry its value as child
/// text (the XISF long form for `String` properties) instead of a `value`
/// attribute.
struct OpenProperty {
    id: Option<String>,
    prop: Property,
    has_value_attr: bool,
}

/// Extract keywords and properties from the decoded XML header.
fn parse_xml(xml: &str) -> Result<Header> {
    let mut reader = Reader::from_str(xml);
    let mut header = Header::new();
    let mut open_property: Option<OpenProperty> = None;

    loop {
        match reader.read_event()? {
            Event::Empty(e) => {
                let local = e.local_name();
                let tag = local.as_ref();
                if tag.eq_ignore_ascii_case(b"FITSKeyword") {
                    header.keywords.push(parse_keyword(&e)?);
                } else if tag.eq_ignore_ascii_case(b"Property") {
                    let (id, prop, _) = parse_property(&e)?;
                    if let Some(id) = id {
                        header.properties.insert(id, prop);
                    }
                }
            }
            Event::Start(e) => {
                let local = e.local_name();
                let tag = local.as_ref();
                if tag.eq_ignore_ascii_case(b"FITSKeyword") {
                    header.keywords.push(parse_keyword(&e)?);
                } else if tag.eq_ignore_ascii_case(b"Property") {
                    let (id, prop, has_value_attr) = parse_property(&e)?;
                    open_property = Some(OpenProperty {
                        id,
                        prop,
                        has_value_attr,
                    });
                }
            }
            Event::Text(t) => {
                if let Some(open) = open_property.as_mut() {
                    if !open.has_value_attr {
                        let text = t
                            .xml_content(XmlVersion::Implicit1_0)
                            .map_err(quick_xml::Error::from)?;
                        open.prop.value.push_str(&text);
                    }
                }
            }
            Event::CData(c) => {
                if let Some(open) = open_property.as_mut() {
                    if !open.has_value_attr {
                        let text = c.decode().map_err(quick_xml::Error::from)?;
                        open.prop.value.push_str(&text);
                    }
                }
            }
            Event::End(e) if e.local_name().as_ref().eq_ignore_ascii_case(b"Property") => {
                if let Some(OpenProperty {
                    id: Some(id), prop, ..
                }) = open_property.take()
                {
                    header.properties.insert(id, prop);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    Ok(header)
}

/// Read a `<FITSKeyword name= value= comment=>` element.
fn parse_keyword(e: &BytesStart) -> Result<FitsKeyword> {
    let mut kw = FitsKeyword::default();
    for attr in e.attributes() {
        let attr = attr?;
        let value = attr.normalized_value(XmlVersion::Implicit1_0)?;
        match attr.key.as_ref() {
            k if k.eq_ignore_ascii_case(b"name") => kw.name = value.into_owned(),
            k if k.eq_ignore_ascii_case(b"value") => {
                kw.value = classify_value(&value);
            }
            k if k.eq_ignore_ascii_case(b"comment") => kw.comment = value.into_owned(),
            _ => {}
        }
    }
    Ok(kw)
}

/// Read a `<Property>` element's attributes: `id`, `type`, `value`,
/// `comment`, and `format`, all kept verbatim (XISF property values are not
/// FITS-quoted). Returns the id (if any), the property, and whether a `value`
/// attribute was present (when absent, the value may follow as child text).
fn parse_property(e: &BytesStart) -> Result<(Option<String>, Property, bool)> {
    let mut id = None;
    let mut prop = Property::default();
    let mut has_value_attr = false;
    for attr in e.attributes() {
        let attr = attr?;
        let raw = attr.normalized_value(XmlVersion::Implicit1_0)?;
        match attr.key.as_ref() {
            k if k.eq_ignore_ascii_case(b"id") => id = Some(raw.into_owned()),
            k if k.eq_ignore_ascii_case(b"type") => prop.type_ = raw.into_owned(),
            k if k.eq_ignore_ascii_case(b"value") => {
                prop.value = raw.into_owned();
                has_value_attr = true;
            }
            k if k.eq_ignore_ascii_case(b"comment") => prop.comment = raw.into_owned(),
            k if k.eq_ignore_ascii_case(b"format") => prop.format = raw.into_owned(),
            _ => {}
        }
    }
    Ok((id, prop, has_value_attr))
}

/// Classify a keyword value attribute: single-quote-wrapped text is a string
/// value (one quote layer stripped); anything else is a bare literal.
fn classify_value(text: &str) -> Value {
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        Value::Str(text[1..text.len() - 1].to_owned())
    } else {
        Value::Literal(text.to_owned())
    }
}
