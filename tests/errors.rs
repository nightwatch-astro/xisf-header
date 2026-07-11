//! Integration tests for error paths on the public parse and file APIs.

mod common;

use common::wrap_container;
use xisf_header::{Error, Header, StructuralHints};

#[test]
fn declared_header_over_cap_is_rejected() {
    let mut bytes = wrap_container("<xisf/>");
    let over = u32::try_from(8 * 1024 * 1024 + 1).unwrap();
    bytes[8..12].copy_from_slice(&over.to_le_bytes());
    assert!(matches!(
        Header::parse(&bytes),
        Err(Error::HeaderTooLarge { .. })
    ));
}

#[test]
fn non_utf8_header_is_rejected() {
    let mut bytes = wrap_container("<xisf></xisf>");
    bytes[18] = 0xFF;
    assert!(matches!(Header::parse(&bytes), Err(Error::Utf8(_))));
}

#[test]
fn malformed_xml_is_rejected() {
    let bytes = wrap_container("<xisf><Image></xisf>");
    assert!(matches!(Header::parse(&bytes), Err(Error::Xml(_))));
}

#[test]
fn read_from_missing_file_is_an_io_error() {
    let path = std::env::temp_dir().join("xisf-header-does-not-exist.xisf");
    assert!(matches!(Header::read_from_file(&path), Err(Error::Io(_))));
}

#[test]
fn read_from_file_with_bad_signature_errors() {
    let path = std::env::temp_dir().join(format!("xisf-header-badsig-{}.xisf", std::process::id()));
    std::fs::write(&path, b"NOTXISF0................").unwrap();
    assert!(matches!(
        Header::read_from_file(&path),
        Err(Error::InvalidSignature)
    ));
    std::fs::remove_file(&path).ok();
}

#[test]
fn update_file_propagates_parse_errors_without_writing() {
    let path =
        std::env::temp_dir().join(format!("xisf-header-badupdate-{}.xisf", std::process::id()));
    std::fs::write(&path, b"garbage that is not xisf").unwrap();
    let result = Header::update_file(&path, &StructuralHints::default(), |h| {
        h.set("OBJECT", "M31").unwrap();
    });
    assert!(result.is_err());
    // The malformed file is left untouched.
    assert_eq!(std::fs::read(&path).unwrap(), b"garbage that is not xisf");
    std::fs::remove_file(&path).ok();
}
