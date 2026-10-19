// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

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
use xisf_header::{Error, Header};

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

fn multi_attachment_xisf(keywords: &str, image: &[u8], thumbnail: &[u8]) -> Vec<u8> {
    const OFFSET_WIDTH: usize = 12;
    const IMAGE_PLACEHOLDER: &str = "IIIIIIIIIIII";
    const THUMB_PLACEHOLDER: &str = "TTTTTTTTTTTT";
    let template = format!(
        "<xisf><Image location=\"attachment:{IMAGE_PLACEHOLDER}:{}\">\n{keywords}\n</Image>\n\
         <Thumbnail location=\"attachment:{THUMB_PLACEHOLDER}:{}\"/></xisf>",
        image.len(),
        thumbnail.len(),
    );
    let image_offset = format!("{:0width$}", 16 + template.len(), width = OFFSET_WIDTH);
    let thumbnail_offset = format!(
        "{:0width$}",
        16 + template.len() + image.len(),
        width = OFFSET_WIDTH
    );
    let xml = template
        .replace(IMAGE_PLACEHOLDER, &image_offset)
        .replace(THUMB_PLACEHOLDER, &thumbnail_offset);
    let mut container = Vec::with_capacity(16 + xml.len() + image.len() + thumbnail.len());
    container.extend_from_slice(b"XISF0100");
    container.extend_from_slice(&u32::try_from(xml.len()).unwrap().to_le_bytes());
    container.extend_from_slice(&[0; 4]);
    container.extend_from_slice(xml.as_bytes());
    container.extend_from_slice(image);
    container.extend_from_slice(thumbnail);
    container
}

fn attachment_locations(container: &[u8]) -> Vec<(usize, usize)> {
    let xml_len = u32::from_le_bytes(container[8..12].try_into().unwrap()) as usize;
    let xml = std::str::from_utf8(&container[16..16 + xml_len]).unwrap();
    xml.match_indices("attachment:")
        .map(|(start, _)| {
            let value_start = start + "attachment:".len();
            let end = value_start + xml[value_start..].find('"').unwrap();
            let (offset, size) = xml[value_start..end].split_once(':').unwrap();
            (offset.parse().unwrap(), size.parse().unwrap())
        })
        .collect()
}

fn patch_attachment_location(container: &mut [u8], index: usize, location: &str) {
    let xml_len = u32::from_le_bytes(container[8..12].try_into().unwrap()) as usize;
    let xml = std::str::from_utf8(&container[16..16 + xml_len]).unwrap();
    let start = xml.match_indices("attachment:").nth(index).unwrap().0;
    let value_start = start + "attachment:".len();
    let end = value_start + xml[value_start..].find('"').unwrap();
    let old_location = &xml[start..end];
    assert_eq!(old_location.len(), location.len());
    container[16 + start..16 + end].copy_from_slice(location.as_bytes());
}

fn multi_attachment_xisf_with_gap_and_reordered_data(image: &[u8], thumbnail: &[u8]) -> Vec<u8> {
    const OFFSET_WIDTH: usize = 12;
    const THUMB_PLACEHOLDER: &str = "TTTTTTTTTTTT";
    const IMAGE_PLACEHOLDER: &str = "IIIIIIIIIIII";
    let gap = b"gap-between-thumbnail-and-image";
    let template = format!(
        "<xisf><Image location=\"attachment:{IMAGE_PLACEHOLDER}:{}\">\n<FITSKeyword name=\"GAIN\" value=\"1\" comment=\"\"/>\n</Image>\n\
         <Thumbnail location=\"attachment:{THUMB_PLACEHOLDER}:{}\"/></xisf>",
        image.len(),
        thumbnail.len(),
    );
    let xml_len = template.len();
    let thumbnail_offset = 16 + xml_len + gap.len();
    let image_offset = thumbnail_offset + thumbnail.len();
    let xml = template
        .replace(
            THUMB_PLACEHOLDER,
            &format!("{thumbnail_offset:0width$}", width = OFFSET_WIDTH),
        )
        .replace(
            IMAGE_PLACEHOLDER,
            &format!("{image_offset:0width$}", width = OFFSET_WIDTH),
        );
    debug_assert_eq!(xml.len(), xml_len);
    let mut container =
        Vec::with_capacity(16 + xml.len() + gap.len() + image.len() + thumbnail.len());
    container.extend_from_slice(b"XISF0100");
    container.extend_from_slice(&u32::try_from(xml.len()).unwrap().to_le_bytes());
    container.extend_from_slice(&[0; 4]);
    container.extend_from_slice(xml.as_bytes());
    container.extend_from_slice(gap);
    container.extend_from_slice(thumbnail);
    container.extend_from_slice(image);
    container
}
fn assert_attachment_data(container: &[u8], expected: &[&[u8]]) {
    let locations = attachment_locations(container);
    assert_eq!(locations.len(), expected.len());
    for ((offset, size), bytes) in locations.into_iter().zip(expected) {
        assert_eq!(size, bytes.len());
        assert_eq!(&container[offset..offset + size], *bytes);
    }
}

#[test]
fn multiple_attachments_noop_is_byte_exact() {
    let image = ramp_data(64);
    let thumbnail: Vec<u8> = (0..19).map(|n| (128 + n) as u8).collect();
    let original = multi_attachment_xisf("", &image, &thumbnail);
    let path = std::env::temp_dir().join(format!(
        "xisf-header-multi-noop-{}.xisf",
        std::process::id()
    ));
    std::fs::write(&path, &original).unwrap();
    Header::update_file(&path, |_| Ok(())).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(&path).ok();
}

#[test]
fn multiple_attachments_preserve_gaps_and_file_order_on_growth() {
    let image = ramp_data(73);
    let thumbnail: Vec<u8> = (0..31).map(|n| (201 - n) as u8).collect();
    let original = multi_attachment_xisf_with_gap_and_reordered_data(&image, &thumbnail);
    let old_locations = attachment_locations(&original);
    assert!(
        old_locations[0].0 > old_locations[1].0,
        "Thumbnail precedes Image data in file"
    );
    let old_gap_start = old_locations[1].0 + old_locations[1].1;
    let old_gap = &original[old_gap_start..old_locations[0].0];
    let original_gap = old_gap.to_vec();
    let path =
        std::env::temp_dir().join(format!("xisf-header-reordered-{}.xisf", std::process::id()));
    std::fs::write(&path, &original).unwrap();
    Header::update_file(&path, |header| {
        header.set("OBJECT", "grow the XML without changing either payload")?;
        Ok(())
    })
    .unwrap();
    let updated = std::fs::read(&path).unwrap();
    let locations = attachment_locations(&updated);
    assert!(
        locations[0].0 > locations[1].0,
        "element order remains independent of file order"
    );
    assert_attachment_data(&updated, &[image.as_slice(), thumbnail.as_slice()]);
    let gap_start = locations[1].0 + locations[1].1;
    assert_eq!(&updated[gap_start..locations[0].0], original_gap.as_slice());
    std::fs::remove_file(&path).ok();
}

#[test]
fn unsafe_attachment_layouts_are_rejected_without_writing() {
    let image = ramp_data(40);
    let thumbnail: Vec<u8> = (0..21).map(|n| (90 + n) as u8).collect();
    let valid = multi_attachment_xisf("", &image, &thumbnail);
    let xml_len = u32::from_le_bytes(valid[8..12].try_into().unwrap()) as usize;
    let image_offset = 16 + xml_len;
    let invalid_locations = [
        ("overlap", format!("attachment:{image_offset:012}:21")),
        ("past-eof", format!("attachment:{:012}:21", valid.len() - 1)),
        ("inside-header", "attachment:000000000016:21".to_owned()),
        ("malformed", "attachment:000000000012:x0".to_owned()),
    ];
    for (label, location) in invalid_locations {
        let mut container = valid.clone();
        patch_attachment_location(&mut container, 1, &location);
        let path = std::env::temp_dir().join(format!(
            "xisf-header-invalid-{label}-{}.xisf",
            std::process::id()
        ));
        std::fs::write(&path, &container).unwrap();
        let result = Header::update_file(&path, |header| {
            header.set("OBJECT", "this edit would grow the header")?;
            Ok(())
        });
        assert!(
            matches!(result, Err(Error::Unsupported(_))),
            "{label}: {result:?}"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            container,
            "{label}: failed edit must not write"
        );
        std::fs::remove_file(&path).ok();
    }
}

#[test]
fn attachment_like_keyword_comment_is_not_treated_as_location() {
    let image = ramp_data(80);
    let keywords = r#"<FITSKeyword name="COMMENT" value="" comment="contains location='attachment:5000:10' text"/>"#;
    let container = mk_xisf(keywords, "", "", &image);
    let path = std::env::temp_dir().join(format!(
        "xisf-header-keyword-location-{}.xisf",
        std::process::id()
    ));
    std::fs::write(&path, &container).unwrap();
    Header::update_file(&path, |header| {
        header.set("OBJECT", "change the header length")?;
        Ok(())
    })
    .unwrap();
    let updated = std::fs::read(&path).unwrap();
    let (image_offset, image_size) = attachment_location(&updated);
    assert_eq!(image_size, image.len());
    assert_eq!(
        &updated[image_offset..image_offset + image_size],
        image.as_slice()
    );
    std::fs::remove_file(&path).ok();
}

#[test]
fn property_attachment_location_is_relocated() {
    let data = ramp_data(52);
    let keywords = r#"<FITSKeyword name="GAIN" value="1" comment=""/>"#;
    let mut container = mk_xisf(
        keywords,
        r#"<Property id="Test:Attachment" type="String" value="x" location="attachment:0000000000:52"/>"#,
        "",
        &data,
    );
    let image_offset = attachment_location(&container).0;
    let property_offset = image_offset + data.len();
    patch_attachment_location(
        &mut container,
        1,
        &format!("attachment:{property_offset:010}:52"),
    );
    container.extend_from_slice(&data);
    let path = std::env::temp_dir().join(format!(
        "xisf-header-property-attachment-{}.xisf",
        std::process::id()
    ));
    std::fs::write(&path, &container).unwrap();
    let old_image_offset = attachment_locations(&container)[0].0;
    Header::update_file(&path, |header| {
        header.set("OBJECT", "move both image and property data")?;
        Ok(())
    })
    .unwrap();
    let updated = std::fs::read(&path).unwrap();
    assert_attachment_data(&updated, &[data.as_slice(), data.as_slice()]);
    let locations = attachment_locations(&updated);
    assert!(locations[0].0 > old_image_offset);
    std::fs::remove_file(&path).ok();
}
#[test]
fn multiple_attachments_survive_growing_and_shrinking_edits() {
    let image = ramp_data(128);
    let thumbnail: Vec<u8> = (0..37).map(|n| (255 - n) as u8).collect();
    let original = multi_attachment_xisf(
        r#"<FITSKeyword name="GAIN" value="1" comment=""/>"#,
        &image,
        &thumbnail,
    );
    let path = std::env::temp_dir().join(format!(
        "xisf-header-multi-attachments-{}.xisf",
        std::process::id()
    ));
    let old_locations = attachment_locations(&original);
    assert!(old_locations[1].0 > old_locations[0].0);
    assert_attachment_data(&original, &[image.as_slice(), thumbnail.as_slice()]);

    std::fs::write(&path, &original).unwrap();
    Header::update_file(&path, |header| {
        header.set(
            "OBJECT",
            "a deliberately long value that grows the XML header",
        )?;
        Ok(())
    })
    .unwrap();
    let grown = std::fs::read(&path).unwrap();
    let grown_locations = attachment_locations(&grown);
    assert_eq!(
        grown_locations.len(),
        2,
        "both locations must parse after growth"
    );
    assert!(grown_locations[0].0 > old_locations[0].0);
    assert!(grown_locations[1].0 > old_locations[1].0);
    assert_attachment_data(&grown, &[image.as_slice(), thumbnail.as_slice()]);

    std::fs::write(&path, &original).unwrap();
    Header::update_file(&path, |header| {
        header.remove("GAIN")?;
        Ok(())
    })
    .unwrap();
    let shrunk = std::fs::read(&path).unwrap();
    let shrunk_locations = attachment_locations(&shrunk);
    assert_eq!(
        shrunk_locations.len(),
        2,
        "both locations must parse after shrinking"
    );
    assert!(shrunk_locations[0].0 < old_locations[0].0);
    assert!(shrunk_locations[1].0 < old_locations[1].0);
    assert_attachment_data(&shrunk, &[image.as_slice(), thumbnail.as_slice()]);
    Header::read_from_file(&path).unwrap();

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

/// Build a monolithic XISF file with an optional leading UTF-8 BOM inside the
/// XML header, `location` deliberately NOT the last `<Image>` attribute (so a
/// misplaced splice would land mid-attribute), and a non-empty gap between
/// the header and the attached data (so offset accounting can't ignore it).
///
/// The `location` offset counts the BOM and the gap, matching a real writer;
/// a fixed-width zero-padded offset keeps the XML length independent of the
/// offset's digit count so it can be substituted after the fact.
fn mk_bom_gapped(with_bom: bool, keywords: &str, data: &[u8], gap: &[u8]) -> Vec<u8> {
    const OFFSET_WIDTH: usize = 10;
    let bom = if with_bom { "\u{FEFF}" } else { "" };
    let template = format!(
        "{bom}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <xisf version=\"1.0\" xmlns=\"http://www.pixinsight.com/xisf\">\n\
         <Metadata><Description>bom fixture</Description></Metadata>\n\
         <Image geometry=\"1:1:{samples}\" sampleFormat=\"UInt8\" \
         location=\"attachment:{{offset}}:{size}\" colorSpace=\"Gray\">\n\
         <Resolution horizontal=\"72\" vertical=\"72\" unit=\"inch\"/>\n\
         {keywords}\n\
         </Image>\n\
         </xisf>\n",
        samples = data.len().max(1),
        size = data.len(),
    );
    let placeholder = "0".repeat(OFFSET_WIDTH);
    let xml_len = template.replace("{offset}", &placeholder).len();
    let offset = 16 + xml_len + gap.len();
    let offset_str = format!("{offset:0width$}", width = OFFSET_WIDTH);
    let xml = template.replace("{offset}", &offset_str);
    assert_eq!(xml.len(), xml_len, "offset width must not change length");

    let mut out = Vec::with_capacity(16 + xml.len() + gap.len() + data.len());
    out.extend_from_slice(b"XISF0100");
    out.extend_from_slice(&(u32::try_from(xml.len()).unwrap()).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(xml.as_bytes());
    out.extend_from_slice(gap);
    out.extend_from_slice(data);
    out
}

/// Regression: a leading UTF-8 BOM must not corrupt spliced edits. quick_xml
/// strips the BOM and reports byte positions relative to the post-BOM
/// content, so without reconciliation every element span would be off by the
/// BOM's 3 bytes and an edit would splice mid-attribute while returning Ok.
#[test]
fn bom_and_gap_no_op_and_edit() {
    let data = ramp_data(64);
    let gap = b"\x00\x00\x00\x00\x00"; // 5-byte alignment gap before the data
    let keywords = r#"<FITSKeyword name="GAIN" value="100" comment="sensor gain"/>
<FITSKeyword name="OBJECT" value="'M31'" comment="target"/>"#;
    let container = mk_bom_gapped(true, keywords, &data, gap);

    // The BOM really is on disk, right after the 16-byte preamble.
    assert_eq!(
        &container[16..19],
        &[0xEF, 0xBB, 0xBF],
        "fixture must carry a BOM"
    );

    let path = std::env::temp_dir().join(format!("xisf-header-bom-{}.xisf", std::process::id()));

    // (a) No-op edit reproduces the BOM'd, gapped file byte-for-byte.
    std::fs::write(&path, &container).unwrap();
    Header::update_file(&path, |_h| Ok(())).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        container,
        "no-op edit on a BOM'd file must be byte-exact"
    );

    // (b) A real edit (which grows the XML, forcing an offset recompute)
    //     produces valid XML: it re-parses with the new value, unmodeled
    //     elements survive, and the attached data is intact at the new offset.
    std::fs::write(&path, &container).unwrap();
    Header::update_file(&path, |h| {
        h.set("GAIN", 20000_i64)?; // wider value: grows the header
        Ok(())
    })
    .unwrap();
    let edited = std::fs::read(&path).unwrap();

    // Still valid, correctly-positioned XML — not a mid-attribute splice.
    let header = Header::read_from_file(&path).unwrap();
    assert_eq!(header.get_i64("GAIN").unwrap(), Some(20000));
    assert_eq!(header.get_str("OBJECT").unwrap(), Some("M31"));
    assert!(
        contains(
            &edited,
            b"<Metadata><Description>bom fixture</Description></Metadata>"
        ),
        "unmodeled Metadata must survive a BOM'd edit verbatim"
    );
    assert!(
        contains(&edited, UNMODELED_RESOLUTION.as_bytes()),
        "unmodeled Resolution must survive a BOM'd edit verbatim"
    );
    // The BOM is preserved and the (relocated) attachment still points at the
    // exact original data bytes — the gap was not swallowed.
    assert_eq!(
        &edited[16..19],
        &[0xEF, 0xBB, 0xBF],
        "BOM must be preserved"
    );
    assert_eq!(
        attachment_data(&edited),
        data.as_slice(),
        "data must survive a BOM'd, gapped, length-changing edit byte-identical"
    );
}

/// A file containing `HISTORY` keywords (in the canonical, spec-conformant
/// `value="" comment="…"` form) must still no-op round-trip byte-exact, and
/// an edit that actually changes a `HISTORY` line must re-render it in that
/// same comment-attribute form (not the old malformed quoted-value form).
#[test]
fn history_keyword_no_op_and_edit_round_trip() {
    let data = ramp_data(48);
    let keywords = r#"<FITSKeyword name="HISTORY" value="" comment="calibrated with WBPP"/>
<FITSKeyword name="HISTORY" value="" comment="registered"/>
<FITSKeyword name="OBJECT" value="'M31'" comment="target"/>"#;
    let container = mk_xisf(keywords, "", UNMODELED_METADATA, &data);
    let path =
        std::env::temp_dir().join(format!("xisf-header-history-{}.xisf", std::process::id()));

    // No-op: byte-exact.
    std::fs::write(&path, &container).unwrap();
    Header::update_file(&path, |_h| Ok(())).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        container,
        "no-op edit on a HISTORY-containing file must be byte-exact"
    );

    // An edit to a HISTORY occurrence re-renders it in the comment-attr form.
    std::fs::write(&path, &container).unwrap();
    Header::update_file(&path, |h| {
        h.set(("HISTORY", 1), "reprocessed")?;
        Ok(())
    })
    .unwrap();
    let edited_bytes = std::fs::read(&path).unwrap();
    let edited_xml = std::str::from_utf8(&edited_bytes).unwrap();
    assert!(
        contains(
            edited_bytes.as_slice(),
            br#"name="HISTORY" value="" comment="reprocessed""#
        ),
        "edited HISTORY must use the comment-attr form: {edited_xml}"
    );
    assert!(!edited_xml.contains("&apos;reprocessed&apos;"));

    let edited_header = Header::read_from_file(&path).unwrap();
    assert_eq!(
        edited_header.get_all::<String>("HISTORY"),
        ["calibrated with WBPP", "reprocessed"]
    );
    assert_eq!(edited_header.get_str("OBJECT").unwrap(), Some("M31"));

    std::fs::remove_file(&path).ok();
}

/// The atomic write follows a symlink (replacing its target, leaving the
/// link a link) and preserves the target's unix permission mode.
#[cfg(unix)]
#[test]
fn update_file_follows_symlink_and_preserves_mode() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("xisf-header-symlink-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("real.xisf");
    let link = dir.join("link.xisf");

    let data = ramp_data(32);
    std::fs::write(
        &target,
        mk_xisf(
            r#"<FITSKeyword name="GAIN" value="1" comment=""/>"#,
            "",
            "",
            &data,
        ),
    )
    .unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    Header::update_file(&link, |h| {
        h.set("GAIN", 2_i64)?;
        Ok(())
    })
    .unwrap();

    // The link is still a symlink to the same target…
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    // …the edit landed on the target…
    assert_eq!(
        Header::read_from_file(&target)
            .unwrap()
            .get_i64("GAIN")
            .unwrap(),
        Some(2)
    );
    // …and the restrictive 0600 mode was not widened.
    assert_eq!(
        std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Regression: a non-empty header↔data gap must be preserved across a
/// length-changing edit — the recomputed offset shifts by the header delta
/// only, so the gap bytes and the data stay put relative to each other.
#[test]
fn gap_preserved_across_length_change() {
    let data = ramp_data(128);
    let gap = b"\xAA\xBB\xCC\xDD"; // distinctive, non-zero gap
    let keywords = r#"<FITSKeyword name="GAIN" value="1" comment=""/>"#;
    let container = mk_bom_gapped(false, keywords, &data, gap);

    let (old_offset, _) = attachment_location(&container);
    let path = std::env::temp_dir().join(format!("xisf-header-gap-{}.xisf", std::process::id()));
    std::fs::write(&path, &container).unwrap();

    Header::update_file(&path, |h| {
        h.set("GAIN", 123456789_i64)?; // grow the header
        Ok(())
    })
    .unwrap();
    let edited = std::fs::read(&path).unwrap();
    let (new_offset, _) = attachment_location(&edited);

    assert!(
        new_offset > old_offset,
        "offset must move later as the header grows"
    );
    // The gap bytes still sit immediately before the (relocated) data.
    assert_eq!(
        &edited[new_offset - gap.len()..new_offset],
        gap,
        "the distinctive gap bytes must survive immediately before the data"
    );
    assert_eq!(
        attachment_data(&edited),
        data.as_slice(),
        "data must be intact at the recomputed, gap-preserving offset"
    );
    std::fs::remove_file(&path).ok();
}
