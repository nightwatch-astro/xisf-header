//! Error and result types for the crate.

use thiserror::Error;

/// A specialized [`Result`](std::result::Result) alias for header operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur while parsing, reading, or writing an XISF header.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// The input was shorter than required (the 16-byte preamble, or the
    /// preamble plus the declared XML-header length).
    #[error("input too small: need at least {needed} bytes, got {got}")]
    TooSmall {
        /// Minimum number of bytes required.
        needed: usize,
        /// Number of bytes actually supplied.
        got: usize,
    },

    /// The first eight bytes were not the `XISF0100` monolithic-signature.
    #[error("invalid XISF signature (expected `XISF0100`)")]
    InvalidSignature,

    /// The declared XML-header length exceeded the 8 MiB safety cap.
    #[error("XML header too large: {len} bytes exceeds the {max}-byte cap")]
    HeaderTooLarge {
        /// Declared header length, in bytes.
        len: usize,
        /// Maximum accepted header length, in bytes.
        max: usize,
    },

    /// The XML header was not valid UTF-8.
    #[error("XML header is not valid UTF-8")]
    Utf8(#[from] std::str::Utf8Error),

    /// The XML header was syntactically malformed.
    #[error("malformed XML header: {0}")]
    Xml(#[from] quick_xml::Error),

    /// An attribute in the XML header was malformed.
    #[error("malformed XML attribute: {0}")]
    Attr(#[from] quick_xml::events::attributes::AttrError),

    /// An I/O error occurred while reading or writing a file.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
