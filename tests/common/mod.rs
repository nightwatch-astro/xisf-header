//! Shared helpers for the integration test suites.

/// Wrap an XML header in a valid 16-byte preamble (no trailing attachment).
pub fn wrap_container(xml: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"XISF0100");
    out.extend_from_slice(&(u32::try_from(xml.len()).unwrap()).to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(xml.as_bytes());
    out
}
