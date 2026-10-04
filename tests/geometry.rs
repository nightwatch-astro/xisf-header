// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Native `<Image geometry>` reads. The geometry comes from the `<Image>`
//! element itself, never from `NAXISn` keywords, and every layout without a
//! single well-formed geometry is an explicit error, never plausible
//! dimensions.

mod common;

use common::{attachment_data, wrap_container};
use xisf_header::{GeometryError, Header, StructuralHints};

/// A monolithic XISF file shaped like a PixInsight save of a 4×4
/// single-channel UInt16 image: native `geometry`, FITS keywords with no
/// `NAXISn`, a `<Metadata>` block, and `data` attached after the header.
fn pixinsight_4x4_file(data: &[u8]) -> Vec<u8> {
    let xml_for = |offset: &str| {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<xisf version="1.0" xmlns="http://www.pixinsight.com/xisf">
<Image geometry="4:4:1" sampleFormat="UInt16" bounds="0:1" colorSpace="Gray" location="attachment:{offset}:{size}">
<FITSKeyword name="IMAGETYP" value="'Light Frame'" comment="Type of exposure"/>
<FITSKeyword name="EXPTIME" value="120." comment="[s] Exposure duration"/>
<FITSKeyword name="FOCALLEN" value="530." comment="[mm] Focal length"/>
<Property id="Instrument:Camera:Name" type="String" value="ZWO ASI2600MM Pro"/>
</Image>
<Metadata>
<Property id="XISF:CreatorApplication" type="String">PixInsight 1.9.3</Property>
</Metadata>
</xisf>
"#,
            size = data.len()
        )
    };
    // Fixed-width offset: the XML length does not depend on its digits.
    let xml_len = xml_for("0000000000").len();
    let xml = xml_for(&format!("{:010}", 16 + xml_len));
    let mut file = wrap_container(&xml);
    file.extend_from_slice(data);
    file
}

/// A parsed header with one `<Image>` element carrying `image_attrs`.
fn image_header(image_attrs: &str) -> Header {
    let xml = format!(
        "<xisf version=\"1.0\" xmlns=\"http://www.pixinsight.com/xisf\">\
         <Image {image_attrs} sampleFormat=\"UInt16\" colorSpace=\"Gray\">\
         <FITSKeyword name=\"IMAGETYP\" value=\"'Light Frame'\" comment=\"\"/>\
         </Image></xisf>"
    );
    Header::parse(&wrap_container(&xml)).unwrap()
}

fn hints(geometry: &str) -> StructuralHints {
    StructuralHints {
        geometry: geometry.to_owned(),
        sample_format: "UInt16".to_owned(),
        color_space: "Gray".to_owned(),
    }
}

/// The reported bug: a real XISF file declares its 4×4 size only through
/// `<Image geometry>`, with no `NAXISn` keywords to fall back on.
#[test]
fn real_file_without_naxis_reports_native_geometry() {
    let data: Vec<u8> = (1..=32).collect(); // 4×4 UInt16 samples
    let path = std::env::temp_dir().join(format!(
        "xisf-header-geometry-4x4-{}.xisf",
        std::process::id()
    ));
    let original = pixinsight_4x4_file(&data);
    std::fs::write(&path, &original).unwrap();

    let header = Header::read_from_file(&path).unwrap();
    let geometry = header.image_geometry().unwrap().unwrap();
    assert_eq!(geometry.dimensions(), &[4, 4]);
    assert_eq!(geometry.channels(), 1);
    assert_eq!(header.image_geometry_raw(), Some("4:4:1"));

    // Keywords and properties are read as declared; none is NAXIS.
    let names: Vec<&str> = header.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(names, ["IMAGETYP", "EXPTIME", "FOCALLEN"]);
    assert_eq!(header.get_f64("FOCALLEN").unwrap(), Some(530.0));
    assert_eq!(
        header.property("XISF:CreatorApplication"),
        Some("PixInsight 1.9.3")
    );

    // Reading is pure: the file is byte-for-byte unchanged.
    assert_eq!(std::fs::read(&path).unwrap(), original);

    // A length-changing in-place edit moves the data block but leaves the
    // geometry and the pixel bytes intact.
    Header::update_file(&path, |h| h.set("OBJECT", "M 31")).unwrap();
    let edited = std::fs::read(&path).unwrap();
    let reread = Header::parse(&edited).unwrap();
    let geometry = reread.image_geometry().unwrap().unwrap();
    assert_eq!(
        (geometry.dimensions(), geometry.channels()),
        (&[4, 4][..], 1)
    );
    assert_eq!(reread.get_str("OBJECT").unwrap(), Some("M 31"));
    assert_eq!(attachment_data(&edited), &data[..]);
    std::fs::remove_file(&path).ok();
}

#[test]
fn every_legal_geometry_form_reads_exactly() {
    let cases: &[(&str, &[u32], u32)] = &[
        ("6248:4176:1", &[6248, 4176], 1),     // monochrome
        ("960:540:3", &[960, 540], 3),         // RGB
        ("960:540:4", &[960, 540], 4),         // RGB + alpha
        ("8:1", &[8], 1),                      // one-dimensional
        ("64:64:16:2", &[64, 64, 16], 2),      // three-dimensional
        ("4294967295:1:1", &[u32::MAX, 1], 1), // u32 boundary
        ("+4: 4 :1", &[4, 4], 1),              // sign, whitespace (§8.3.1)
        ("\u{FEFF}4:4\u{3000}:1", &[4, 4], 1), // ECMAScript \s: U+FEFF, Zs
        ("0x10:0X10:0b1", &[16, 16], 1),       // hex, binary (§8.3.2)
        ("0o10:0x0004:1", &[8, 4], 1),         // octal, zero-padded hex
    ];
    for &(text, dimensions, channels) in cases {
        let header = image_header(&format!("geometry=\"{text}\""));
        let geometry = header
            .image_geometry()
            .unwrap_or_else(|| panic!("{text}: no geometry"))
            .unwrap_or_else(|e| panic!("{text}: {e}"));
        assert_eq!(geometry.dimensions(), dimensions, "{text}");
        assert_eq!(geometry.channels(), channels, "{text}");
        assert_eq!(header.image_geometry_raw(), Some(text), "{text}");
    }

    // The attribute name is case-insensitive, like every other attribute.
    let header = image_header("GEOMETRY=\"4:4:1\"");
    assert_eq!(
        header.image_geometry().unwrap().unwrap().dimensions(),
        &[4, 4]
    );

    // The raw value is XML-decoded: a character reference reads as its digit.
    let header = image_header("geometry=\"&#52;:4:1\"");
    assert_eq!(header.image_geometry_raw(), Some("4:4:1"));
    assert_eq!(
        header.image_geometry().unwrap().unwrap().dimensions(),
        &[4, 4]
    );
}

#[test]
fn malformed_geometry_is_an_error_and_keeps_raw_text() {
    for text in [
        "",                // empty
        "1",               // a channel count with no dimension
        "4:4:",            // empty channel count
        ":4:1",            // empty dimension
        "4::1",            // empty dimension
        "\u{0085}4:4:1",   // U+0085 is not ECMAScript \s
        "0:4:1",           // zero width
        "4:0:1",           // zero height
        "4:4:0",           // zero channels
        "-4:4:1",          // negative
        "-0:4:1",          // negative zero
        "4294967296:4:1",  // past u32::MAX
        "0x100000000:1:1", // past u32::MAX, hex
        "04:4:1",          // decimal leading zero
        "4.0:4:1",         // not an integer
        "4 4:1",           // inner whitespace
        "four:4:1",        // not a number
        "0x:4:1",          // radix prefix without digits
        "+0x4:4:1",        // sign on a radix form
    ] {
        let header = image_header(&format!("geometry=\"{text}\""));
        assert_eq!(
            header.image_geometry(),
            Some(Err(GeometryError::Malformed)),
            "{text:?}"
        );
        assert_eq!(header.image_geometry_raw(), Some(text), "{text:?}");
        assert_eq!(
            header.get_str("IMAGETYP").unwrap(),
            Some("Light Frame"),
            "{text:?}"
        );
    }
}

#[test]
fn image_without_geometry_attribute_is_missing() {
    let header = image_header("");
    assert_eq!(header.image_geometry(), Some(Err(GeometryError::Missing)));
    assert_eq!(header.image_geometry_raw(), None);
}

#[test]
fn unreadable_image_attributes_are_malformed_not_a_parse_failure() {
    for attrs in [
        r#"geometry="4:4:1" geometry="8:8:1""#,
        r#"geometry="4:4:1" GEOMETRY="8:8:1""#,
        r#"geometry="4:4:1" bounds="0:1" bounds="0:1""#,
        r#"geometry="4&bogus;4:1""#,
    ] {
        let header = image_header(attrs);
        assert_eq!(
            header.image_geometry(),
            Some(Err(GeometryError::Malformed)),
            "{attrs}"
        );
        assert_eq!(header.image_geometry_raw(), None, "{attrs}");
        assert_eq!(
            header.get_str("IMAGETYP").unwrap(),
            Some("Light Frame"),
            "{attrs}"
        );
    }
}

#[test]
fn multiple_images_are_an_error_never_the_first_image() {
    for (first, second) in [("4:4:1", "4:4:1"), ("4:4:1", "8:8:3"), ("0:0:0", "4:4:1")] {
        let xml = format!(
            "<xisf version=\"1.0\" xmlns=\"http://www.pixinsight.com/xisf\">\
             <Image geometry=\"{first}\" sampleFormat=\"UInt16\" colorSpace=\"Gray\">\
             <FITSKeyword name=\"IMAGETYP\" value=\"'Light Frame'\" comment=\"\"/></Image>\
             <Image geometry=\"{second}\" sampleFormat=\"UInt16\" colorSpace=\"Gray\"/>\
             </xisf>"
        );
        let header = Header::parse(&wrap_container(&xml)).unwrap();
        assert_eq!(
            header.image_geometry(),
            Some(Err(GeometryError::MultipleImages)),
            "{first} + {second}"
        );
        assert_eq!(header.image_geometry_raw(), None, "{first} + {second}");
    }
}

#[test]
fn header_without_image_element_has_no_geometry() {
    let xml = "<xisf version=\"1.0\" xmlns=\"http://www.pixinsight.com/xisf\">\
               <Metadata><Property id=\"XISF:CreatorApplication\" type=\"String\" value=\"x\"/></Metadata>\
               </xisf>";
    let header = Header::parse(&wrap_container(xml)).unwrap();
    assert_eq!(header.image_geometry(), None);
    assert_eq!(header.image_geometry_raw(), None);
}

/// Geometry describes the container, not the header model: headers written
/// with different hints still compare equal, to each other and to the
/// in-memory original.
#[test]
fn geometry_is_not_part_of_header_equality() {
    let mut original = Header::new();
    original.set("IMAGETYP", "Master Flat").unwrap();
    let mono = Header::parse(&original.to_header_bytes(&hints("4:4:1"))).unwrap();
    let colour = Header::parse(&original.to_header_bytes(&hints("8:8:3"))).unwrap();

    assert_eq!(mono.image_geometry_raw(), Some("4:4:1"));
    assert_eq!(colour.image_geometry_raw(), Some("8:8:3"));
    assert_eq!(mono, colour);
    assert_eq!(mono, original);
}
