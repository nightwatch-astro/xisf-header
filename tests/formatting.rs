//! Integration tests for controlled value formatting through a full
//! write → parse cycle.

use xisf_header::{Fixed, Header, Literal, Sci, StructuralHints};

fn round_trip(h: &Header) -> Header {
    Header::parse(&h.to_header_bytes(&StructuralHints::default())).unwrap()
}

#[test]
fn fixed_point_formatting_survives_round_trip() {
    let mut h = Header::new();
    h.set("EXPTIME", Fixed(300.0, 2)).unwrap();
    h.set("XPIXSZ", Fixed(3.76, 1)).unwrap();
    let parsed = round_trip(&h);
    assert_eq!(parsed.get_str("EXPTIME").unwrap(), Some("300.00"));
    assert_eq!(parsed.get_str("XPIXSZ").unwrap(), Some("3.8"));
    assert_eq!(parsed.get_f64("EXPTIME").unwrap(), Some(300.0));
}

#[test]
fn scientific_formatting_survives_round_trip() {
    let mut h = Header::new();
    h.set("FLUX", Sci(1234.5, 3)).unwrap();
    h.set("TINY", Sci(0.00012345, 2)).unwrap();
    let parsed = round_trip(&h);
    assert_eq!(parsed.get_str("FLUX").unwrap(), Some("1.23E3"));
    assert_eq!(parsed.get_str("TINY").unwrap(), Some("1.2E-4"));
    assert_eq!(parsed.get_f64("FLUX").unwrap(), Some(1230.0));
}

#[test]
fn literal_escape_hatch_is_emitted_verbatim() {
    let mut h = Header::new();
    h.set("VENDOR", Literal("0x1F".to_owned())).unwrap();
    let parsed = round_trip(&h);
    assert_eq!(parsed.get_str("VENDOR").unwrap(), Some("0x1F"));
    // A literal is not a string value: no quote layer appears in the XML.
    let bytes = h.to_header_bytes(&StructuralHints::default());
    let xml = std::str::from_utf8(&bytes[16..]).unwrap();
    assert!(xml.contains(r#"value="0x1F""#));
}

#[test]
fn default_f64_always_reads_back_as_float() {
    let mut h = Header::new();
    h.set("SCALE", 2.0).unwrap();
    let parsed = round_trip(&h);
    assert_eq!(parsed.get_str("SCALE").unwrap(), Some("2.0"));
    assert_eq!(parsed.get_f64("SCALE").unwrap(), Some(2.0));
}
