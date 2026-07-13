//! End-to-end tests against real, standards-valid monolithic XISF files:
//! preamble + XML header (with an unmodeled element the crate never parses)
//! plus a real, non-zero attached data block. Covers three header-size
//! regimes (a handful of keywords, dozens, and hundreds) to exercise the
//! splice path at different scales.
//!
//! This is the crate's core promise under test: `update_file` is byte-exact
//! and data-preserving, in contrast to the old `to_bytes`/`write_to_file`
//! behavior it replaced (zero-filled data, unmodeled XML dropped).

mod common;

use common::{attachment_data, attachment_location, mk_xisf};
use xisf_header::Header;

/// An XML fragment the parser never models — must survive every edit
/// verbatim, since it's the whole point of byte-exact splicing.
const UNMODELED_METADATA: &str = "<Metadata><Description>Integration test fixture</Description>\
     <CreationTime value=\"2026-07-13T00:00:00Z\"/></Metadata>";

/// Also unmodeled, and nested inside `<Image>` (baked into every
/// `mk_xisf` container) rather than beside it.
const UNMODELED_RESOLUTION: &str = "<Resolution horizontal=\"72\" vertical=\"72\" unit=\"inch\"/>";

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

fn assert_unmodeled_survives(bytes: &[u8], label: &str) {
    assert!(
        contains(bytes, UNMODELED_METADATA.as_bytes()),
        "{label}: unmodeled <Metadata> block must survive verbatim"
    );
    assert!(
        contains(bytes, UNMODELED_RESOLUTION.as_bytes()),
        "{label}: unmodeled <Resolution> element must survive verbatim"
    );
}

fn build_keywords_xml(n: usize) -> String {
    (0..n)
        .map(|i| format!(r#"<FITSKeyword name="K{i}" value="{i}" comment="c{i}"/>"#))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_properties_xml(n: usize) -> String {
    (0..n)
        .map(|i| format!(r#"<Property id="Test:Prop{i}" type="Int32" value="{i}"/>"#))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A ramp pattern (not all-zero, not all-same-byte) so any accidental
/// truncation, shift, or zero-fill is easy to detect.
fn ramp_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8).collect()
}

/// First, middle, and last index in `0..n` (deduplicated), for spot checks.
fn spot_indices(n: usize) -> Vec<usize> {
    let mut v = vec![0, n / 2, n - 1];
    v.dedup();
    v
}

/// Exercise the full read/no-op/edit/grow/shrink matrix for a header with
/// `n_keywords` keywords and `n_properties` properties.
fn check_regime(label: &str, n_keywords: usize, n_properties: usize) {
    let keywords_xml = build_keywords_xml(n_keywords);
    let properties_xml = build_properties_xml(n_properties);
    let data = ramp_data(4096);
    let container = mk_xisf(&keywords_xml, &properties_xml, UNMODELED_METADATA, &data);

    let path = std::env::temp_dir().join(format!(
        "xisf-header-it-{label}-{}.xisf",
        std::process::id()
    ));
    let write = |bytes: &[u8]| std::fs::write(&path, bytes).unwrap();

    // 1. Parse/read_from_file reads the modeled header correctly.
    write(&container);
    let header = Header::read_from_file(&path).unwrap();
    assert_eq!(
        header.keywords().len(),
        n_keywords,
        "{label}: keyword count"
    );
    assert_eq!(
        header.properties().len(),
        n_properties,
        "{label}: property count"
    );
    for i in spot_indices(n_keywords) {
        assert_eq!(
            header.get_i64(format!("K{i}").as_str()).unwrap(),
            Some(i as i64),
            "{label}: keyword K{i}"
        );
    }
    for i in spot_indices(n_properties) {
        assert_eq!(
            header.property(&format!("Test:Prop{i}")),
            Some(i.to_string().as_str()),
            "{label}: property Test:Prop{i}"
        );
    }

    // 2. No-op update_file reproduces the whole file byte-for-byte.
    write(&container);
    Header::update_file(&path, |_h| Ok(())).unwrap();
    let after_noop = std::fs::read(&path).unwrap();
    assert_eq!(
        after_noop, container,
        "{label}: no-op edit must reproduce the file byte-for-byte"
    );

    // 3. A keyword edit changes only that keyword: unmodeled content and the
    //    attached data block survive exactly.
    write(&container);
    let original_data = attachment_data(&container).to_vec();
    Header::update_file(&path, |h| {
        h.set("K0", 999_i64)?;
        Ok(())
    })
    .unwrap();
    let edited_bytes = std::fs::read(&path).unwrap();
    let edited_header = Header::read_from_file(&path).unwrap();
    assert_eq!(
        edited_header.get_i64("K0").unwrap(),
        Some(999),
        "{label}: edited keyword value"
    );
    for i in spot_indices(n_keywords).into_iter().filter(|&i| i != 0) {
        assert_eq!(
            edited_header.get_i64(format!("K{i}").as_str()).unwrap(),
            Some(i as i64),
            "{label}: untouched keyword K{i} after edit"
        );
    }
    assert_eq!(
        attachment_data(&edited_bytes),
        original_data.as_slice(),
        "{label}: data block must survive a keyword edit byte-identical"
    );
    assert_unmodeled_survives(&edited_bytes, label);

    // 4. An edit that grows the XML (many new keywords) relocates the
    //    attachment OFFSET but keeps SIZE and the data bytes exact.
    write(&container);
    let (old_offset, old_size) = attachment_location(&container);
    Header::update_file(&path, |h| {
        for i in 0..50 {
            h.set(format!("NEW{i}").as_str(), i as i64)?;
        }
        Ok(())
    })
    .unwrap();
    let grown_bytes = std::fs::read(&path).unwrap();
    let (grown_offset, grown_size) = attachment_location(&grown_bytes);
    assert_eq!(
        grown_size, old_size,
        "{label}: SIZE must not change on grow"
    );
    assert!(
        grown_offset > old_offset,
        "{label}: OFFSET must move later when the header grows"
    );
    assert_eq!(
        attachment_data(&grown_bytes),
        data.as_slice(),
        "{label}: data bytes must survive a growing edit byte-identical"
    );
    assert_unmodeled_survives(&grown_bytes, label);
    let grown_header = Header::read_from_file(&path).unwrap();
    assert_eq!(grown_header.get_i64("NEW0").unwrap(), Some(0));
    assert_eq!(grown_header.get_i64("NEW49").unwrap(), Some(49));
    assert_eq!(grown_header.keywords().len(), n_keywords + 50);

    // 5. An edit that shrinks the XML (remove keywords) likewise preserves
    //    data + unmodeled content.
    write(&container);
    let to_remove = n_keywords / 2;
    Header::update_file(&path, |h| {
        for i in 0..to_remove {
            h.remove(format!("K{i}").as_str())?;
        }
        Ok(())
    })
    .unwrap();
    let shrunk_bytes = std::fs::read(&path).unwrap();
    let (shrunk_offset, shrunk_size) = attachment_location(&shrunk_bytes);
    assert_eq!(
        shrunk_size, old_size,
        "{label}: SIZE must not change on shrink"
    );
    if to_remove > 0 {
        assert!(
            shrunk_offset < old_offset,
            "{label}: OFFSET must move earlier when the header shrinks"
        );
    }
    assert_eq!(
        attachment_data(&shrunk_bytes),
        data.as_slice(),
        "{label}: data bytes must survive a shrinking edit byte-identical"
    );
    assert_unmodeled_survives(&shrunk_bytes, label);
    let shrunk_header = Header::read_from_file(&path).unwrap();
    assert_eq!(shrunk_header.keywords().len(), n_keywords - to_remove);
    for i in to_remove..n_keywords {
        assert_eq!(
            shrunk_header.get_i64(format!("K{i}").as_str()).unwrap(),
            Some(i as i64),
            "{label}: surviving keyword K{i} after shrink"
        );
    }

    std::fs::remove_file(&path).ok();
}

#[test]
fn small_header_regime() {
    check_regime("small", 4, 2);
}

#[test]
fn normal_header_regime() {
    check_regime("normal", 40, 15);
}

#[test]
fn oversized_header_regime() {
    check_regime("oversized", 300, 120);
}
