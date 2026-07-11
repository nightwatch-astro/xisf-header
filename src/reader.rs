//! Parsing: preamble validation and XML extraction into a [`Header`].

use std::fs::File;
use std::io::Read;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::{Error, Result};
use crate::header::Header;
use crate::keyword::FitsKeyword;
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

/// Extract keywords and properties from the decoded XML header.
fn parse_xml(xml: &str) -> Result<Header> {
    let mut reader = Reader::from_str(xml);
    let mut header = Header::new();

    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) => {
                let local = e.local_name();
                let tag = local.as_ref();
                if tag.eq_ignore_ascii_case(b"FITSKeyword") {
                    header.keywords.push(parse_keyword(&e)?);
                } else if tag.eq_ignore_ascii_case(b"Property") {
                    if let Some((id, value)) = parse_property(&e)? {
                        header.properties.insert(id, value);
                    }
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

/// Read a `<Property id= value=>` element, returning `None` if it has no `id`.
fn parse_property(e: &BytesStart) -> Result<Option<(String, String)>> {
    let mut id = None;
    let mut value = String::new();
    for attr in e.attributes() {
        let attr = attr?;
        let raw = attr.normalized_value(XmlVersion::Implicit1_0)?;
        match attr.key.as_ref() {
            k if k.eq_ignore_ascii_case(b"id") => id = Some(raw.into_owned()),
            k if k.eq_ignore_ascii_case(b"value") => value = strip_fits_quotes(&raw).to_owned(),
            _ => {}
        }
    }
    Ok(id.map(|id| (id, value)))
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

/// Strip exactly one layer of FITS single-quote wrapping, if present.
fn strip_fits_quotes(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        &s[1..s.len() - 1]
    } else {
        s
    }
}
