// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Integration test against a realistic PixInsight-style XISF header,
//! including elements this crate does not model (Metadata, Resolution,
//! Thumbnail) that must not break parsing.

mod common;

use common::wrap_container;
use xisf_header::{Header, StructuralHints};

/// A representative monolithic-XISF header as produced by PixInsight-like
/// writers: namespaced root, Metadata block, typed properties (attribute and
/// child-text forms), FITS keywords with quoted strings, and sibling elements.
const PIXINSIGHT_STYLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xisf version="1.0" xmlns="http://www.pixinsight.com/xisf"
      xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
      xsi:schemaLocation="http://www.pixinsight.com/xisf http://pixinsight.com/xisf/xisf-1.0.xsd">
  <Metadata>
    <Property id="XISF:CreatorApplication" type="String">PixInsight 1.8.9</Property>
    <Property id="XISF:CreationTime" type="TimePoint" value="2026-07-11T22:15:03.000Z"/>
  </Metadata>
  <Image geometry="6248:4176:1" sampleFormat="UInt16" colorSpace="Gray"
         location="attachment:4096:52175616">
    <Resolution horizontal="72" vertical="72" unit="inch"/>
    <FITSKeyword name="SIMPLE" value="T" comment="file does conform to FITS standard"/>
    <FITSKeyword name="IMAGETYP" value="'Light Frame'" comment="Type of exposure"/>
    <FITSKeyword name="EXPTIME" value="300." comment="[s] Total integration time"/>
    <FITSKeyword name="GAIN" value="100" comment="Sensor gain"/>
    <FITSKeyword name="DATE-OBS" value="'2026-07-11T22:15:03.123'" comment="UTC start of exposure"/>
    <FITSKeyword name="OBJECT" value="'NGC 7000'" comment="Target"/>
    <FITSKeyword name="HISTORY" value="" comment="calibrated with WBPP"/>
    <FITSKeyword name="HISTORY" value="" comment="registered"/>
    <Property id="Instrument:Telescope:FocalLength" type="Float32" value="0.53"/>
    <Property id="Instrument:Sensor:Temperature" type="Float32" value="-10.0"/>
    <Property id="Observation:Object:Name" type="String">NGC 7000</Property>
    <Thumbnail geometry="256:171:1" sampleFormat="UInt8" colorSpace="Gray"
               location="attachment:52179712:43776"/>
  </Image>
</xisf>
"#;

#[test]
fn realistic_header_parses_fully() {
    let h = Header::parse(&wrap_container(PIXINSIGHT_STYLE)).unwrap();

    // Keywords, including the quoted-string convention and repeats.
    assert_eq!(h.keywords().len(), 8);
    assert_eq!(h.get_bool("SIMPLE").unwrap(), Some(true));
    assert_eq!(h.get_str("IMAGETYP").unwrap(), Some("Light Frame"));
    assert_eq!(h.get_f64("EXPTIME").unwrap(), Some(300.0));
    assert_eq!(h.get_u32("GAIN").unwrap(), Some(100));
    assert_eq!(h.get_str("OBJECT").unwrap(), Some("NGC 7000"));
    let dt = h.get_datetime("DATE-OBS").unwrap().unwrap();
    assert_eq!((dt.year(), dt.month() as u8, dt.day()), (2026, 7, 11));
    assert_eq!(h.count("HISTORY"), 2);

    // Properties from both the Metadata block and the Image, in both forms.
    assert_eq!(
        h.property("XISF:CreatorApplication"),
        Some("PixInsight 1.8.9")
    );
    assert_eq!(h.properties()["XISF:CreationTime"].type_, "TimePoint");
    assert_eq!(
        h.property_get::<f64>("Instrument:Telescope:FocalLength"),
        Some(0.53)
    );
    assert_eq!(h.property("Observation:Object:Name"), Some("NGC 7000"));
}

#[test]
fn realistic_header_edit_round_trip_keeps_fidelity() {
    let mut h = Header::parse(&wrap_container(PIXINSIGHT_STYLE)).unwrap();

    h.set("OBJECT", "North America Nebula").unwrap();
    h.remove("GAIN").unwrap();

    let reparsed = Header::parse(&h.to_header_bytes(&StructuralHints::default())).unwrap();
    assert_eq!(reparsed, h);

    // The edit took effect…
    assert_eq!(
        reparsed.get_str("OBJECT").unwrap(),
        Some("North America Nebula")
    );
    assert_eq!(reparsed.get_str("GAIN").unwrap(), None);

    // …and untouched keywords and typed properties are unchanged.
    assert_eq!(reparsed.get_str("IMAGETYP").unwrap(), Some("Light Frame"));
    let p = &reparsed.properties()["Instrument:Sensor:Temperature"];
    assert_eq!((p.type_.as_str(), p.value.as_str()), ("Float32", "-10.0"));
    assert_eq!(
        reparsed.properties()["XISF:CreationTime"].type_,
        "TimePoint"
    );
}
