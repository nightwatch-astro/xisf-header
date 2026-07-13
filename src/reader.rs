//! Parsing: preamble validation and XML extraction into a [`Header`].

use std::fs::File;
use std::io::Read;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::{Error, Result};
use crate::header::Header;
use crate::keyword::{is_commentary, FitsKeyword};
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
    ///
    /// ```
    /// use xisf_header::{Header, StructuralHints};
    ///
    /// let mut header = Header::new();
    /// header.set("IMAGETYP", "Master Dark")?;
    ///
    /// let bytes = header.to_header_bytes(&StructuralHints::default());
    /// let parsed = Header::parse(&bytes)?;
    /// assert_eq!(parsed.get_str("IMAGETYP")?, Some("Master Dark"));
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let (start, end) = split_preamble(bytes)?;
        let xml = std::str::from_utf8(&bytes[start..end])?;
        parse_xml(xml)
    }

    /// Read and parse the header of an XISF file, reading only the preamble and
    /// XML header — never the pixel/attachment payload.
    ///
    /// # Errors
    ///
    /// Propagates I/O errors and any [`Header::parse`] error.
    ///
    /// ```
    /// use xisf_header::{Header, StructuralHints};
    ///
    /// let path = std::env::temp_dir().join("xisf-header-doctest-read.xisf");
    /// let mut header = Header::new();
    /// header.set("IMAGETYP", "Master Dark")?;
    /// std::fs::write(&path, header.to_header_bytes(&StructuralHints::default()))?;
    ///
    /// let reloaded = Header::read_from_file(&path)?;
    /// assert_eq!(reloaded.get_str("IMAGETYP")?, Some("Master Dark"));
    /// # std::fs::remove_file(&path).ok();
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
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

/// Validate the 16-byte preamble and return the byte range `(start, end)` of
/// the UTF-8 XML header within `bytes` (`start` is always 16).
pub(crate) fn split_preamble(bytes: &[u8]) -> Result<(usize, usize)> {
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
    Ok((16, end))
}

/// A `<Property>` opened as a start tag, which may carry its value as child
/// text (the XISF long form for `String` properties) instead of a `value`
/// attribute.
struct OpenProperty {
    id: Option<String>,
    prop: Property,
    has_value_attr: bool,
    /// Byte offset (into the XML `&str`) where the opening `<Property` tag
    /// began, so [`XmlIndex::property_spans`] can record the whole element.
    span_start: usize,
}

/// Byte spans of the modeled elements in a parsed XML header, plus the
/// single attachment location if the layout is one [`crate::writer`]'s
/// byte-exact `update_file` splice can target. Spans are byte offsets into
/// the XML `&str` passed to [`parse_xml_with_index`].
pub(crate) struct XmlIndex {
    /// Span of each named `<FITSKeyword>` element, in document order —
    /// aligned 1:1 with [`Header::keywords`] by index, since both are built
    /// by the same skip-if-nameless traversal.
    pub keyword_spans: Vec<(usize, usize)>,
    /// Span of each id'd `<Property>` element, in document order, paired
    /// with its id (nameless `<Property>`s are skipped, like keywords).
    pub property_spans: Vec<(String, usize, usize)>,
    /// The data-bearing element's location and insertion point, or an error
    /// reason when the document's attachment layout isn't one the splice
    /// path can safely target.
    pub image: std::result::Result<ImageInfo, String>,
}

/// Where a single `<Image location="attachment:OFFSET:SIZE">` element's
/// attachment lives, and where to splice in newly-appended elements.
pub(crate) struct ImageInfo {
    /// Span of the `attachment:OFFSET:SIZE` text (excluding the surrounding
    /// quotes) within the `location` attribute.
    pub location_span: (usize, usize),
    pub offset: usize,
    pub size: usize,
    /// Byte offset just before `</Image>`, where new `<FITSKeyword>`/
    /// `<Property>` elements are appended. `None` when `<Image>` is
    /// self-closing and so has no child content to insert into.
    pub insertion_point: Option<usize>,
}

/// Extract keywords and properties from the decoded XML header, and
/// [`XmlIndex`] and [`Header`] together.
fn parse_xml(xml: &str) -> Result<Header> {
    parse_xml_with_index(xml).map(|(header, _)| header)
}

/// Like [`parse_xml`], but also records the byte spans needed to splice a
/// byte-exact update (see [`crate::splice`]).
pub(crate) fn parse_xml_with_index(xml: &str) -> Result<(Header, XmlIndex)> {
    let mut reader = Reader::from_str(xml);
    let mut header = Header::new();
    let mut open_property: Option<OpenProperty> = None;
    let mut keyword_spans = Vec::new();
    let mut property_spans = Vec::new();

    let mut image_count = 0usize;
    let mut image_end_start: Option<usize> = None;
    // (is_image, location value span, offset, size), for every element
    // (other than FITSKeyword/Property) carrying a valid `attachment:N:N`
    // `location` attribute — used to reject layouts with zero or multiple
    // attachments.
    let mut locations: Vec<(bool, usize, usize, usize, usize)> = Vec::new();

    loop {
        let start = reader.buffer_position() as usize;
        let event = reader.read_event()?;
        let end = reader.buffer_position() as usize;
        match event {
            Event::Empty(e) => {
                let local = e.local_name();
                let tag = local.as_ref();
                if tag.eq_ignore_ascii_case(b"FITSKeyword") {
                    if let Some(kw) = parse_keyword(&e)? {
                        header.keywords.push(kw);
                        keyword_spans.push((start, end));
                    }
                } else if tag.eq_ignore_ascii_case(b"Property") {
                    let (id, prop, _) = parse_property(&e)?;
                    if let Some(id) = id {
                        property_spans.push((id.clone(), start, end));
                        header.properties.insert(id, prop);
                    }
                } else {
                    if tag.eq_ignore_ascii_case(b"Image") {
                        image_count += 1;
                    }
                    record_location(xml, start, end, tag, &mut locations);
                }
            }
            Event::Start(e) => {
                let local = e.local_name();
                let tag = local.as_ref();
                if tag.eq_ignore_ascii_case(b"FITSKeyword") {
                    if let Some(kw) = parse_keyword(&e)? {
                        header.keywords.push(kw);
                        keyword_spans.push((start, end));
                    }
                } else if tag.eq_ignore_ascii_case(b"Property") {
                    let (id, prop, has_value_attr) = parse_property(&e)?;
                    open_property = Some(OpenProperty {
                        id,
                        prop,
                        has_value_attr,
                        span_start: start,
                    });
                } else {
                    if tag.eq_ignore_ascii_case(b"Image") {
                        image_count += 1;
                    }
                    record_location(xml, start, end, tag, &mut locations);
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
            Event::End(e) => {
                let local = e.local_name();
                let tag = local.as_ref();
                if tag.eq_ignore_ascii_case(b"Property") {
                    if let Some(open) = open_property.take() {
                        if let Some(id) = open.id {
                            property_spans.push((id.clone(), open.span_start, end));
                            header.properties.insert(id, open.prop);
                        }
                    }
                } else if tag.eq_ignore_ascii_case(b"Image") {
                    image_end_start = Some(start);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    let image = resolve_image(image_count, &locations, image_end_start);
    Ok((
        header,
        XmlIndex {
            keyword_spans,
            property_spans,
            image,
        },
    ))
}

/// If `tag` carries a `location="attachment:OFFSET:SIZE"` attribute, record
/// it. Only called for elements other than `FITSKeyword`/`Property`, which
/// never legitimately carry `location` (and whose attribute *values* could
/// otherwise coincidentally contain the text `location=`).
fn record_location(
    xml: &str,
    start: usize,
    end: usize,
    tag: &[u8],
    locations: &mut Vec<(bool, usize, usize, usize, usize)>,
) {
    let Some((value_start, value_end)) =
        find_attr_value_span(&xml.as_bytes()[start..end], b"location")
    else {
        return;
    };
    let (abs_start, abs_end) = (start + value_start, start + value_end);
    let Some((offset, size)) = xml
        .get(abs_start..abs_end)
        .and_then(parse_attachment_location)
    else {
        return;
    };
    locations.push((
        tag.eq_ignore_ascii_case(b"Image"),
        abs_start,
        abs_end,
        offset,
        size,
    ));
}

/// Decide whether the document has a splice-able single attachment: exactly
/// one `<Image>` element, carrying the document's one `location="attachment:
/// …">` attribute.
fn resolve_image(
    image_count: usize,
    locations: &[(bool, usize, usize, usize, usize)],
    image_end_start: Option<usize>,
) -> std::result::Result<ImageInfo, String> {
    if image_count == 0 {
        return Err("no <Image> element found".to_owned());
    }
    if image_count > 1 {
        return Err(format!(
            "found {image_count} <Image> elements; update_file supports exactly one"
        ));
    }
    if locations.len() != 1 {
        return Err(format!(
            "found {} attachment location(s); update_file supports exactly one",
            locations.len()
        ));
    }
    let (is_image, value_start, value_end, offset, size) = locations[0];
    if !is_image {
        return Err("the attachment location is not on the <Image> element".to_owned());
    }
    Ok(ImageInfo {
        location_span: (value_start, value_end),
        offset,
        size,
        insertion_point: image_end_start,
    })
}

/// Parse an `attachment:OFFSET:SIZE` location value.
fn parse_attachment_location(value: &str) -> Option<(usize, usize)> {
    let rest = value.strip_prefix("attachment:")?;
    let (offset, size) = rest.split_once(':')?;
    Some((offset.parse().ok()?, size.parse().ok()?))
}

/// Find the byte span of `attr_name`'s value (excluding quotes) within a
/// single start-tag's raw bytes, case-insensitive on the name. XISF
/// attribute values never contain markup, so a plain scan (rather than a
/// full attribute tokenizer) is sufficient.
fn find_attr_value_span(tag: &[u8], attr_name: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + attr_name.len() <= tag.len() {
        if !tag[i..i + attr_name.len()].eq_ignore_ascii_case(attr_name) {
            i += 1;
            continue;
        }
        let before_ok = i == 0 || tag[i - 1].is_ascii_whitespace();
        let mut j = i + attr_name.len();
        if before_ok {
            while j < tag.len() && tag[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < tag.len() && tag[j] == b'=' {
                j += 1;
                while j < tag.len() && tag[j].is_ascii_whitespace() {
                    j += 1;
                }
                if let Some(&quote) = tag.get(j).filter(|&&b| b == b'"' || b == b'\'') {
                    let value_start = j + 1;
                    if let Some(rel_end) = tag[value_start..].iter().position(|&b| b == quote) {
                        return Some((value_start, value_start + rel_end));
                    }
                }
            }
        }
        i += attr_name.len();
    }
    None
}

/// Read a `<FITSKeyword name= value= comment=>` element. An element without a
/// `name` attribute yields `None`: a nameless keyword cannot be addressed and
/// is skipped, like a `<Property>` without an `id`.
///
/// `HISTORY`/`COMMENT` are FITS commentary keywords with no value — their
/// free text is read from `comment` (the correct, spec-conformant XISF form:
/// `value="" comment="text"`). If `comment` is empty, fall back to `value`
/// (unquoted) so files this crate wrote before this fix — which quoted the
/// text into `value` — still read their text back; see [`is_commentary`].
fn parse_keyword(e: &BytesStart) -> Result<Option<FitsKeyword>> {
    let mut name = String::new();
    let mut raw_value = String::new();
    let mut comment = String::new();
    for attr in e.attributes() {
        let attr = attr?;
        let value = attr.normalized_value(XmlVersion::Implicit1_0)?;
        match attr.key.as_ref() {
            k if k.eq_ignore_ascii_case(b"name") => name = value.into_owned(),
            k if k.eq_ignore_ascii_case(b"value") => raw_value = value.into_owned(),
            k if k.eq_ignore_ascii_case(b"comment") => comment = value.into_owned(),
            _ => {}
        }
    }
    if name.is_empty() {
        return Ok(None);
    }
    let (value, comment) = if is_commentary(&name) {
        if comment.is_empty() {
            (classify_value(&raw_value), String::new())
        } else {
            (Value::Str(comment), String::new())
        }
    } else {
        (classify_value(&raw_value), comment)
    };
    Ok(Some(FitsKeyword {
        name,
        value,
        comment,
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Wrap XML in a valid preamble, with explicit reserved bytes.
    fn container(xml: &str, reserved: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(SIGNATURE);
        out.extend_from_slice(&(u32::try_from(xml.len()).unwrap()).to_le_bytes());
        out.extend_from_slice(&reserved);
        out.extend_from_slice(xml.as_bytes());
        out
    }

    #[test]
    fn classify_quoted_and_bare() {
        assert_eq!(classify_value("'M31'"), Value::Str("M31".to_owned()));
        assert_eq!(classify_value("''"), Value::Str(String::new()));
        // One quote layer is stripped, inner quotes stay.
        assert_eq!(classify_value("'''"), Value::Str("'".to_owned()));
        assert_eq!(classify_value("300"), Value::Literal("300".to_owned()));
        assert_eq!(classify_value("'"), Value::Literal("'".to_owned()));
        assert_eq!(classify_value(""), Value::Literal(String::new()));
    }

    #[test]
    fn too_small_preamble() {
        assert!(matches!(
            Header::parse(b"XISF01"),
            Err(Error::TooSmall { needed: 16, got: 6 })
        ));
    }

    #[test]
    fn too_small_for_declared_length() {
        let mut bytes = container("<xisf/>", [0; 4]);
        bytes[8..12].copy_from_slice(&100_u32.to_le_bytes()); // declare more than present
        assert!(matches!(
            Header::parse(&bytes),
            Err(Error::TooSmall { needed: 116, .. })
        ));
    }

    #[test]
    fn header_too_large_is_rejected() {
        let mut bytes = container("<xisf/>", [0; 4]);
        let over = u32::try_from(MAX_HEADER_LEN + 1).unwrap();
        bytes[8..12].copy_from_slice(&over.to_le_bytes());
        assert!(matches!(
            Header::parse(&bytes),
            Err(Error::HeaderTooLarge { max, .. }) if max == MAX_HEADER_LEN
        ));
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let mut bytes = container("<xisf></xisf>", [0; 4]);
        bytes[20] = 0xFF;
        assert!(matches!(Header::parse(&bytes), Err(Error::Utf8(_))));
    }

    #[test]
    fn malformed_xml_is_rejected() {
        let bytes = container("<xisf><Image></xisf>", [0; 4]);
        assert!(matches!(Header::parse(&bytes), Err(Error::Xml(_))));
    }

    #[test]
    fn reserved_bytes_are_ignored() {
        let bytes = container("<xisf/>", [0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(Header::parse(&bytes).is_ok());
    }

    #[test]
    fn attribute_names_are_case_insensitive() {
        let xml = r#"<xisf><FITSKeyword NAME="GAIN" VALUE="100" COMMENT="c"/></xisf>"#;
        let h = Header::parse(&container(xml, [0; 4])).unwrap();
        assert_eq!(h.get_i64("GAIN").unwrap(), Some(100));
        assert_eq!(h.keywords()[0].comment, "c");
    }

    #[test]
    fn nameless_keywords_are_skipped() {
        let xml = r#"<xisf>
            <FITSKeyword value="'orphan'" comment="no name"/>
            <FITSKeyword name="GAIN" value="100" comment=""/>
        </xisf>"#;
        let h = Header::parse(&container(xml, [0; 4])).unwrap();
        assert_eq!(h.keywords().len(), 1);
        assert_eq!(h.get_i64("GAIN").unwrap(), Some(100));
    }

    #[test]
    fn commentary_keyword_reads_from_comment_attribute() {
        // The canonical, spec-conformant form: no value, text in `comment`.
        let xml = r#"<xisf><FITSKeyword name="HISTORY" value="" comment="processed in PixInsight"/></xisf>"#;
        let h = Header::parse(&container(xml, [0; 4])).unwrap();
        assert_eq!(
            h.get_str("HISTORY").unwrap(),
            Some("processed in PixInsight")
        );
    }

    #[test]
    fn commentary_keyword_falls_back_to_quoted_value_for_backward_compat() {
        // Files this crate wrote before this fix quoted the text into
        // `value` and left `comment` empty; those must keep reading.
        let xml =
            r#"<xisf><FITSKeyword name="HISTORY" value="&apos;old form&apos;" comment=""/></xisf>"#;
        let h = Header::parse(&container(xml, [0; 4])).unwrap();
        assert_eq!(h.get_str("HISTORY").unwrap(), Some("old form"));
    }

    #[test]
    fn unknown_attributes_and_elements_are_skipped() {
        let xml = r#"<xisf>
            <Metadata><Property id="XISF:CreatorApplication" type="String" value="PixInsight"/></Metadata>
            <Image geometry="256:256:1" sampleFormat="UInt16" colorSpace="Gray" location="attachment:4096:131072">
                <FITSKeyword name="GAIN" value="100" comment="" unknown="x"/>
                <Resolution horizontal="72" vertical="72"/>
            </Image>
        </xisf>"#;
        let h = Header::parse(&container(xml, [0; 4])).unwrap();
        assert_eq!(h.get_i64("GAIN").unwrap(), Some(100));
        // Properties are collected wherever they appear in the header.
        assert_eq!(h.property("XISF:CreatorApplication"), Some("PixInsight"));
    }
}
