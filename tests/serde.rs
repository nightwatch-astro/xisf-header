//! Serde round-trip tests (only built with `--features serde`).
#![cfg(feature = "serde")]

use xisf_header::{Header, Property, StructuralHints};

fn sample() -> Header {
    let mut h = Header::new();
    h.set("IMAGETYP", "Master Dark").unwrap();
    h.set_comment("IMAGETYP", "Type of image").unwrap();
    h.set("EXPTIME", 300.0).unwrap();
    h.set("SIMPLE", true).unwrap();
    h.append("HISTORY", "a").unwrap();
    h.append("HISTORY", "b").unwrap();
    h.set_property_with_type("Instrument:Telescope:FocalLength", "0.135", "Float32")
        .unwrap();
    h
}

#[test]
fn header_round_trips_through_json() {
    let h = sample();
    let json = serde_json::to_string(&h).unwrap();
    let back: Header = serde_json::from_str(&json).unwrap();
    assert_eq!(back, h);

    // The revived header is fully functional.
    assert_eq!(back.get_f64("EXPTIME").unwrap(), Some(300.0));
    assert_eq!(back.count("HISTORY"), 2);
    assert_eq!(
        back.properties()["Instrument:Telescope:FocalLength"].type_,
        "Float32"
    );
}

#[test]
fn property_type_field_serializes_as_type() {
    let p = Property::new("Float32", "0.135");
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains(r#""type":"Float32""#), "{json}");
}

#[test]
fn structural_hints_round_trip_through_json() {
    let hints = StructuralHints::default();
    let json = serde_json::to_string(&hints).unwrap();
    let back: StructuralHints = serde_json::from_str(&json).unwrap();
    assert_eq!(back, hints);
}

#[test]
fn serialized_header_survives_container_round_trip_too() {
    let h = sample();
    let container = h.to_bytes(&StructuralHints::default());
    let parsed = Header::parse(&container).unwrap();
    let json = serde_json::to_string(&parsed).unwrap();
    let back: Header = serde_json::from_str(&json).unwrap();
    assert_eq!(back, h);
}
