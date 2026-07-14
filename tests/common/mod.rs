// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Shared helpers for the integration test suites.
//!
//! Each `tests/*.rs` binary compiles its own copy of this module via `mod
//! common;`, so a helper only some binaries call looks unused from any one
//! binary's perspective.
#![allow(dead_code)]

/// Wrap an XML header in a valid 16-byte preamble (no trailing attachment).
pub fn wrap_container(xml: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"XISF0100");
    out.extend_from_slice(&(u32::try_from(xml.len()).unwrap()).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(xml.as_bytes());
    out
}

/// Build a standards-valid, byte-exact monolithic XISF file: the 16-byte
/// preamble, a real `<xisf>` header with `keywords`/`properties` XML
/// fragments inside `<Image>`, `extra_xml` as a `<Image>` sibling (e.g. a
/// `<Metadata>` block), and a fixed unmodeled `<Resolution>` element inside
/// `<Image>` — then `data` attached at the declared `location` offset.
///
/// A fixed-width, zero-padded offset field keeps the XML's own length
/// independent of the offset's digit count (the same two-pass trick
/// `Header::to_header_bytes` uses), so the offset can be substituted after
/// the surrounding XML length is known.
pub fn mk_xisf(keywords: &str, properties: &str, extra_xml: &str, data: &[u8]) -> Vec<u8> {
    const OFFSET_WIDTH: usize = 10;
    let template = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <xisf version=\"1.0\" xmlns=\"http://www.pixinsight.com/xisf\">\n\
         {extra_xml}\n\
         <Image geometry=\"1:1:{samples}\" sampleFormat=\"UInt8\" colorSpace=\"Gray\" \
         location=\"attachment:{{offset}}:{size}\">\n\
         <Resolution horizontal=\"72\" vertical=\"72\" unit=\"inch\"/>\n\
         {keywords}\n\
         {properties}\n\
         </Image>\n\
         </xisf>\n",
        samples = data.len().max(1),
        size = data.len(),
    );
    let placeholder = "0".repeat(OFFSET_WIDTH);
    let xml_len = template.replace("{offset}", &placeholder).len();
    let offset_str = format!("{:0width$}", 16 + xml_len, width = OFFSET_WIDTH);
    let xml = template.replace("{offset}", &offset_str);
    debug_assert_eq!(xml.len(), xml_len, "offset width must not change length");

    let mut out = Vec::with_capacity(16 + xml.len() + data.len());
    out.extend_from_slice(b"XISF0100");
    out.extend_from_slice(&(u32::try_from(xml.len()).unwrap()).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(xml.as_bytes());
    out.extend_from_slice(data);
    out
}

/// Read the `attachment:OFFSET:SIZE` location out of a container's XML, by
/// direct string search — independent of `xisf_header`'s own parsing, so
/// tests can verify the attached data block without relying on the crate
/// under test to report its own offset.
pub fn attachment_location(container: &[u8]) -> (usize, usize) {
    let xml_len = u32::from_le_bytes(container[8..12].try_into().unwrap()) as usize;
    let xml = std::str::from_utf8(&container[16..16 + xml_len]).unwrap();
    let after = xml.split("attachment:").nth(1).expect("no location attr");
    let loc = &after[..after.find('"').expect("unterminated location attr")];
    let (offset, size) = loc.split_once(':').expect("malformed location attr");
    (offset.parse().unwrap(), size.parse().unwrap())
}

/// The attached data block, sliced out via [`attachment_location`].
pub fn attachment_data(container: &[u8]) -> &[u8] {
    let (offset, size) = attachment_location(container);
    &container[offset..offset + size]
}
