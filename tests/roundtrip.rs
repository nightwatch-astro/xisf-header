//! Integration tests for the `xisf-header` crate.

use xisf_header::{Error, Header, StructuralHints};

/// A representative header exercising string + numeric keywords and a property.
fn sample() -> Header {
    let mut h = Header::new();
    h.set("IMAGETYP", "Master Dark").unwrap();
    h.set_comment("IMAGETYP", "Type of image").unwrap();
    h.set("EXPTIME", 300.0).unwrap();
    h.set("GAIN", 100_i64).unwrap();
    h.set("OFFSET", 50_i64).unwrap();
    h.set_property("Instrument:Telescope:FocalLength", "0.135")
        .unwrap();
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
    let mut bytes = sample().to_header_bytes(&StructuralHints::default());
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
    let parsed = Header::parse(&h.to_header_bytes(&StructuralHints::default())).unwrap();
    assert_eq!(parsed, h);
}

#[test]
fn string_and_literal_kinds_round_trip() {
    let mut h = Header::new();
    h.set("OBJECT", "a < b & \"c\"").unwrap(); // string, XML specials
    h.set("NAXIS", 2_i64).unwrap(); // bare literal
    let parsed = Header::parse(&h.to_header_bytes(&StructuralHints::default())).unwrap();
    assert_eq!(parsed.get_str("OBJECT").unwrap(), Some("a < b & \"c\""));
    assert_eq!(parsed.get_i64("NAXIS").unwrap(), Some(2));
    assert_eq!(parsed, h);
}

#[test]
fn strict_duplicate_access() {
    let mut h = Header::new();
    h.set("OBJECT", "M31").unwrap();
    assert_eq!(h.get_str("object").unwrap(), Some("M31")); // case-insensitive
    h.set("OBJECT", "M42").unwrap(); // unique update
    assert_eq!(h.get_str("OBJECT").unwrap(), Some("M42"));
    assert_eq!(h.keywords().len(), 1);

    // Build a repeated keyword; bare-name access is now ambiguous.
    h.append("HISTORY", "a").unwrap();
    h.append("HISTORY", "b").unwrap();
    assert!(matches!(
        h.get_str("HISTORY"),
        Err(Error::Ambiguous { count: 2, .. })
    ));
    assert_eq!(h.get_str(("HISTORY", 0)).unwrap(), Some("a"));
    assert_eq!(h.get_str(("HISTORY", 1)).unwrap(), Some("b"));
    assert_eq!(h.count("HISTORY"), 2);
    assert_eq!(h.get_all::<String>("HISTORY"), vec!["a", "b"]);

    // Mutations on the ambiguous name are blocked; select an occurrence.
    assert!(matches!(
        h.set("HISTORY", "x"),
        Err(Error::Ambiguous { .. })
    ));
    h.set(("HISTORY", 1), "b2").unwrap();
    assert_eq!(h.get_str(("HISTORY", 1)).unwrap(), Some("b2"));
    assert!(matches!(h.remove("HISTORY"), Err(Error::Ambiguous { .. })));
    assert!(h.remove(("HISTORY", 0)).unwrap());
    assert_eq!(h.count("HISTORY"), 1);
    assert!(h.remove("HISTORY").unwrap()); // now unique
    assert_eq!(h.count("HISTORY"), 0);
}

#[test]
fn commentary_keywords_round_trip_through_bytes() {
    let mut h = Header::new();
    h.append("HISTORY", "reduced with siril").unwrap();
    h.append("HISTORY", "stacked 20x300s").unwrap();
    h.append("HISTORY", "registered").unwrap();
    h.append("COMMENT", "processed in PixInsight").unwrap();

    let bytes = h.to_header_bytes(&StructuralHints::default());
    let parsed = Header::parse(&bytes).unwrap();

    assert_eq!(parsed.count("HISTORY"), 3);
    assert_eq!(parsed.count("COMMENT"), 1);
    assert_eq!(
        parsed.get_all::<String>("HISTORY"),
        ["reduced with siril", "stacked 20x300s", "registered"]
    );
    assert_eq!(
        parsed.get_all::<String>("COMMENT"),
        ["processed in PixInsight"]
    );
    assert_eq!(parsed, h);
}

#[test]
fn index_out_of_range_errors() {
    let mut h = Header::new();
    h.append("HISTORY", "a").unwrap();
    assert!(matches!(
        h.get_str(("HISTORY", 3)),
        Err(Error::IndexOutOfRange { count: 1, .. })
    ));
    // A missing keyword by index is absence, not an error.
    assert_eq!(h.get_str(("MISSING", 0)).unwrap(), None);
}

#[test]
fn typed_getters_and_lenient_int() {
    let mut h = Header::new();
    h.set("EXPTIME", 300.0).unwrap();
    h.set("GAIN", 100_i64).unwrap();
    h.set("XBINNING", 2_u32).unwrap();
    h.set("SIMPLE", true).unwrap();
    h.set("COUNT", "20.0").unwrap(); // string that reads as an int
    assert_eq!(h.get_f64("EXPTIME").unwrap(), Some(300.0));
    assert_eq!(h.get_i64("GAIN").unwrap(), Some(100));
    assert_eq!(h.get_u32("XBINNING").unwrap(), Some(2));
    assert_eq!(h.get_bool("SIMPLE").unwrap(), Some(true));
    assert_eq!(h.get_i64("COUNT").unwrap(), Some(20));
    assert_eq!(h.get_f64("MISSING").unwrap(), None);
}

#[test]
fn datetime_reads() {
    let mut h = Header::new();
    h.set("DATE-OBS", "2026-07-11T22:15:03").unwrap();
    h.set("DATE-END", "2026-07-11T22:15:03.5").unwrap();
    let dt = h.get_datetime("DATE-OBS").unwrap().unwrap();
    assert_eq!(dt.year(), 2026);
    assert_eq!(dt.hour(), 22);
    assert_eq!(dt.second(), 3);
    assert!(h.get_datetime("DATE-END").unwrap().is_some());
}

#[test]
fn property_crud() {
    let mut h = Header::new();
    let id = "Instrument:Telescope:FocalLength";
    h.set_property(id, "0.135").unwrap();
    assert_eq!(h.property(id), Some("0.135"));
    assert_eq!(h.property_get::<f64>(id), Some(0.135));
    assert!(h.remove_property(id));
    assert!(h.property(id).is_none());
    assert!(matches!(
        h.set_property("bad id!", "x"),
        Err(Error::InvalidName { .. })
    ));
}

#[test]
fn invalid_names_rejected() {
    let mut h = Header::new();
    assert!(matches!(
        h.set("TOOLONGNAME", "x"),
        Err(Error::InvalidName { .. })
    ));
    assert!(matches!(
        h.append("bad key", "x"),
        Err(Error::InvalidName { .. })
    ));
    assert!(h.set("lower", "x").is_ok()); // any-case ≤8 ASCII is fine
}

#[test]
fn batch_mutations_are_atomic() {
    let mut h = Header::new();
    h.set("GAIN", 1_i64).unwrap();
    h.set_many([("FILTER", "Ha"), ("OBJECT", "M31")]).unwrap();
    assert_eq!(h.get_str("FILTER").unwrap(), Some("Ha"));
    assert_eq!(h.get_str("OBJECT").unwrap(), Some("M31"));

    // One invalid entry rejects the whole batch.
    let before = h.keywords().len();
    assert!(h
        .set_many([("TELESCOP", "AA"), ("TOOLONGKEY", "x")])
        .is_err());
    assert_eq!(h.keywords().len(), before);
    assert_eq!(h.get_str("TELESCOP").unwrap(), None);

    assert_eq!(h.remove_many(["FILTER", "OBJECT"]).unwrap(), 2);
}

#[test]
fn quoted_string_values_are_unwrapped() {
    let xml = r#"<xisf><Image location="attachment:0:0">
        <FITSKeyword name="OBJECT" value="'M31'" comment="Target"/>
        <FITSKeyword name="EXPTIME" value="300" comment="[s]"/>
        </Image></xisf>"#;
    let h = Header::parse(&wrap_container(xml)).unwrap();
    assert_eq!(h.get_str("OBJECT").unwrap(), Some("M31"));
    assert_eq!(h.get_i64("EXPTIME").unwrap(), Some(300));
}

#[test]
fn header_only_output_and_file_round_trip() {
    let hints = StructuralHints::default();
    let path = std::env::temp_dir().join(format!("xisf-header-it-{}.xisf", std::process::id()));

    let h = sample();

    // Header-only output parses back and carries no data block.
    let header_only = h.to_header_bytes(&hints);
    assert_eq!(Header::parse(&header_only).unwrap(), h);

    // Assembling a full container means appending the caller's own data,
    // sized per `hints` (1x1x1 UInt8 default = 1 byte).
    let mut container = header_only.clone();
    container.push(0xEE);
    std::fs::write(&path, &container).unwrap();
    assert_eq!(Header::read_from_file(&path).unwrap(), h);

    Header::update_file(&path, |header| {
        header.set("OBJECT", "M31")?;
        header.remove("OFFSET")?;
        Ok(())
    })
    .unwrap();
    let edited = Header::read_from_file(&path).unwrap();
    assert_eq!(edited.get_str("OBJECT").unwrap(), Some("M31"));
    assert_eq!(edited.get_str("OFFSET").unwrap(), None);
    // The pixel byte survives the in-place edit untouched.
    assert_eq!(std::fs::read(&path).unwrap().last(), Some(&0xEE));

    std::fs::remove_file(&path).ok();
}
