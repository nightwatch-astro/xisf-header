//! Integration tests for XISF `<Property>` fidelity.

mod common;

use common::wrap_container;
use xisf_header::{Error, Header, Property, StructuralHints};

#[test]
fn typed_property_round_trips_with_type_and_value_intact() {
    let xml = r#"<xisf><Image location="attachment:0:0">
        <Property id="Instrument:Telescope:FocalLength" type="Float32" value="0.135"/>
        <Property id="Observation:Time:Start" type="TimePoint" value="2026-07-11T22:15:03Z"/>
        </Image></xisf>"#;
    let h = Header::parse(&wrap_container(xml)).unwrap();

    let p = &h.properties()["Instrument:Telescope:FocalLength"];
    assert_eq!(p.type_, "Float32");
    assert_eq!(p.value, "0.135");
    assert_eq!(
        h.property_get::<f64>("Instrument:Telescope:FocalLength"),
        Some(0.135)
    );

    // Round-trip: type stays Float32, value stays unquoted.
    let reparsed = Header::parse(&h.to_bytes(&StructuralHints::default())).unwrap();
    assert_eq!(reparsed, h);
    let p2 = &reparsed.properties()["Instrument:Telescope:FocalLength"];
    assert_eq!(p2.type_, "Float32");
    assert_eq!(p2.value, "0.135");
    assert_eq!(
        reparsed.properties()["Observation:Time:Start"].type_,
        "TimePoint"
    );
}

#[test]
fn property_values_are_not_fits_quoted() {
    let mut h = Header::new();
    h.set_property("Observer:Name", "Sjors").unwrap();
    let bytes = h.to_bytes(&StructuralHints::default());
    let xml = std::str::from_utf8(&bytes[16..]).unwrap();
    assert!(xml.contains(r#"value="Sjors""#), "unexpected XML: {xml}");
    assert!(
        !xml.contains("'Sjors'"),
        "value must not gain a quote layer: {xml}"
    );

    let reparsed = Header::parse(&bytes).unwrap();
    assert_eq!(reparsed.property("Observer:Name"), Some("Sjors"));
}

#[test]
fn child_text_string_property_parses() {
    let xml = r#"<xisf><Image location="attachment:0:0">
        <Property id="Processing:History" type="String">stacked 20x300s with siril</Property>
        </Image></xisf>"#;
    let h = Header::parse(&wrap_container(xml)).unwrap();
    assert_eq!(
        h.property("Processing:History"),
        Some("stacked 20x300s with siril")
    );
    assert_eq!(h.properties()["Processing:History"].type_, "String");
}

#[test]
fn child_text_cdata_property_parses() {
    let xml = r#"<xisf><Image location="attachment:0:0">
        <Property id="Notes" type="String"><![CDATA[a < b & "c"]]></Property>
        </Image></xisf>"#;
    let h = Header::parse(&wrap_container(xml)).unwrap();
    assert_eq!(h.property("Notes"), Some(r#"a < b & "c""#));
}

#[test]
fn value_attribute_wins_over_child_whitespace() {
    let xml = "<xisf><Image location=\"attachment:0:0\">\n\
        <Property id=\"Weather:Temp\" type=\"Float32\" value=\"-3.5\">\n</Property>\n\
        </Image></xisf>";
    let h = Header::parse(&wrap_container(xml)).unwrap();
    assert_eq!(h.property("Weather:Temp"), Some("-3.5"));
}

#[test]
fn comment_and_format_attributes_are_preserved() {
    let xml = r#"<xisf><Image location="attachment:0:0">
        <Property id="Instrument:Camera:Gain" type="Int32" value="100" format="%d" comment="sensor gain"/>
        </Image></xisf>"#;
    let h = Header::parse(&wrap_container(xml)).unwrap();
    let reparsed = Header::parse(&h.to_bytes(&StructuralHints::default())).unwrap();
    let p = &reparsed.properties()["Instrument:Camera:Gain"];
    assert_eq!(p.format, "%d");
    assert_eq!(p.comment, "sensor gain");
    assert_eq!(p.type_, "Int32");
}

#[test]
fn set_property_preserves_existing_type() {
    let mut h = Header::new();
    h.set_property_with_type("Instrument:ExposureTime", "300", "Float32")
        .unwrap();
    h.set_property("Instrument:ExposureTime", "600").unwrap();
    let p = &h.properties()["Instrument:ExposureTime"];
    assert_eq!(p.type_, "Float32");
    assert_eq!(p.value, "600");
}

#[test]
fn set_property_with_type_creates_and_updates() {
    let mut h = Header::new();
    h.set_property("Observer:Name", "Sjors").unwrap();
    assert_eq!(h.properties()["Observer:Name"].type_, "String");

    h.set_property_with_type("Observer:Name", "Sjors", "String8")
        .unwrap();
    assert_eq!(h.properties()["Observer:Name"].type_, "String8");

    assert!(matches!(
        h.set_property_with_type("bad id!", "x", "String"),
        Err(Error::InvalidName { .. })
    ));
}

#[test]
fn property_without_id_is_skipped() {
    let xml = r#"<xisf><Image location="attachment:0:0">
        <Property type="String" value="orphan"/>
        <Property id="Kept" type="String" value="ok"/>
        </Image></xisf>"#;
    let h = Header::parse(&wrap_container(xml)).unwrap();
    assert_eq!(h.properties().len(), 1);
    assert_eq!(h.property("Kept"), Some("ok"));
}

#[test]
fn property_new_constructor() {
    let p = Property::new("Float64", "1.5");
    assert_eq!(p.type_, "Float64");
    assert_eq!(p.value, "1.5");
    assert!(p.comment.is_empty());
    assert!(p.format.is_empty());
}
