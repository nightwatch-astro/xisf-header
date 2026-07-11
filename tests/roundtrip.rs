//! Integration tests for the `xisf-header` crate.

use xisf_header::{Error, FitsKeyword, Header};

/// A representative header exercising keywords (string + numeric) and a property.
fn sample() -> Header {
    let mut h = Header::new();
    h.set("IMAGETYP", "Master Dark", "Type of image");
    h.set("EXPTIME", "300.0", "[s] Exposure time");
    h.extend([
        FitsKeyword::new("GAIN", "100", ""),
        FitsKeyword::new("OFFSET", "50", ""),
    ]);
    h.set_property("Instrument:Telescope:FocalLength", "0.135");
    h
}

/// Wrap an XML header in a valid 16-byte preamble (no trailing attachment).
fn wrap_container(xml: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"XISF0100");
    out.extend_from_slice(&(u32::try_from(xml.len()).unwrap()).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(xml.as_bytes());
    out
}

#[test]
fn bad_signature_errors() {
    let mut bytes = sample().to_bytes();
    bytes[0] = b'Z';
    assert!(matches!(
        Header::parse(&bytes),
        Err(Error::InvalidSignature)
    ));
}

#[test]
fn truncated_input_errors() {
    assert!(matches!(
        Header::parse(b"XISF01"),
        Err(Error::TooSmall { .. })
    ));
}

#[test]
fn round_trips_through_bytes() {
    let h = sample();
    let parsed = Header::parse(&h.to_bytes()).unwrap();
    assert_eq!(parsed, h);
}

#[test]
fn written_container_is_self_consistent() {
    let bytes = sample().to_bytes();
    assert_eq!(&bytes[0..8], b"XISF0100");
    let xml_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    assert_eq!(&bytes[12..16], &[0, 0, 0, 0]);
    // Preamble + XML header + at least the 1-byte placeholder attachment.
    assert!(bytes.len() > 16 + xml_len);
    // The declared header slice is valid UTF-8 XML.
    assert!(std::str::from_utf8(&bytes[16..16 + xml_len]).is_ok());
}

#[test]
fn crud_single_and_bulk() {
    let mut h = Header::new();

    // Create + case-insensitive read.
    h.set("OBJECT", "M31", "Target");
    assert_eq!(h.get_str("object"), Some("M31"));

    // Update in place (no duplicate).
    h.set("OBJECT", "M42", "");
    assert_eq!(h.get_str("OBJECT"), Some("M42"));
    assert_eq!(h.keywords().len(), 1);

    // Bulk insert allowing duplicate names.
    h.extend([
        FitsKeyword::new("GAIN", "100", ""),
        FitsKeyword::new("GAIN", "200", ""),
    ]);
    assert_eq!(h.get_all("GAIN").count(), 2);
    assert_eq!(h.get_i64("GAIN"), Some(100)); // first match wins

    // Delete.
    assert_eq!(h.remove_all("GAIN"), 2);
    assert!(h.get("GAIN").is_none());
    assert!(h.remove("OBJECT"));
    assert!(!h.remove("OBJECT"));
}

#[test]
fn typed_getters() {
    let mut h = Header::new();
    h.set("EXPTIME", "300.0", "");
    h.set("GAIN", "100", "");
    h.set("SIMPLE", "T", "");
    assert_eq!(h.get_f64("EXPTIME"), Some(300.0));
    assert_eq!(h.get_i64("GAIN"), Some(100));
    assert_eq!(h.get_bool("SIMPLE"), Some(true));
    assert_eq!(h.get_f64("MISSING"), None);
}

#[test]
fn property_crud_and_fallbacks() {
    let mut h = Header::new();
    let id = "Instrument:Telescope:FocalLength";
    h.set_property(id, "0.135");
    assert_eq!(h.property(id), Some("0.135"));
    assert_eq!(h.property_f64(id), Some(0.135));
    assert!(h.remove_property(id));
    assert!(h.property(id).is_none());
}

#[test]
fn quoted_string_values_are_unwrapped() {
    // Real XISF string values are single-quote-wrapped on disk; numerics are not.
    let xml = r#"<xisf><Image location="attachment:0:0">
        <FITSKeyword name="OBJECT" value="'M31'" comment="Target"/>
        <FITSKeyword name="EXPTIME" value="300" comment="[s]"/>
        </Image></xisf>"#;
    let h = Header::parse(&wrap_container(xml)).unwrap();
    assert_eq!(h.get_str("OBJECT"), Some("M31"));
    assert_eq!(h.get_i64("EXPTIME"), Some(300));
}

#[test]
fn escaped_attribute_values_round_trip() {
    let mut h = Header::new();
    h.set("COMMENT", "a < b & c \"d\"", "has <special> chars");
    let parsed = Header::parse(&h.to_bytes()).unwrap();
    assert_eq!(parsed.get_str("COMMENT"), Some("a < b & c \"d\""));
    assert_eq!(parsed, h);
}

#[test]
fn file_round_trip_and_update() {
    let path = std::env::temp_dir().join(format!("xisf-header-it-{}.xisf", std::process::id()));

    let h = sample();
    h.write_to_file(&path).unwrap();
    let reloaded = Header::read_from_file(&path).unwrap();
    assert_eq!(reloaded, h);

    Header::update_file(&path, |header| {
        header.set("OBJECT", "M31", "Target");
        header.remove("OFFSET");
    })
    .unwrap();

    let edited = Header::read_from_file(&path).unwrap();
    assert_eq!(edited.get_str("OBJECT"), Some("M31"));
    assert!(edited.get("OFFSET").is_none());

    std::fs::remove_file(&path).ok();
}
