//! Error and result types for the crate.

use thiserror::Error;

/// A specialized [`Result`](std::result::Result) alias for header operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur while parsing, reading, or writing an XISF header.
///
/// ```
/// use xisf_header::{Error, Header};
///
/// let mut header = Header::new();
/// header.append("HISTORY", "reduced with siril").unwrap();
/// header.append("HISTORY", "stacked 20x300s").unwrap();
///
/// assert!(matches!(
///     header.get_str("HISTORY"),
///     Err(Error::Ambiguous { count: 2, .. })
/// ));
/// ```
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

    /// A singular access (`get`/`set`/`remove` by bare name) targeted a keyword
    /// that appears more than once. Disambiguate with an `(name, n)` key, or use
    /// [`get_all`](crate::Header::get_all)/[`count`](crate::Header::count).
    #[error("keyword `{name}` is ambiguous: it appears {count} times")]
    Ambiguous {
        /// The keyword name.
        name: String,
        /// Number of occurrences.
        count: usize,
    },

    /// A [`Key::Nth`](crate::Key::Nth) `(name, n)` access referenced an
    /// occurrence index that does not exist.
    #[error("keyword `{name}` has no occurrence {index} ({count} present)")]
    IndexOutOfRange {
        /// The keyword name.
        name: String,
        /// The requested occurrence index.
        index: usize,
        /// Number of occurrences present.
        count: usize,
    },

    /// A write supplied a name that is not a valid FITS keyword (≤ 8 printable
    /// ASCII characters) or valid XISF property id.
    #[error("invalid identifier `{name}`: {reason}")]
    InvalidName {
        /// The rejected identifier.
        name: String,
        /// Why it was rejected.
        reason: &'static str,
    },

    /// An I/O error occurred while reading or writing a file.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// [`Header::update_file`](crate::Header::update_file) cannot safely
    /// splice this file's XML: the common case is exactly one `<Image
    /// location="attachment:OFFSET:SIZE">` element. Multiple attachments
    /// (e.g. a `Thumbnail` alongside the `Image`), no attachment at all, or a
    /// self-closing `<Image/>` that needs new child elements inserted are
    /// rejected rather than risking data loss.
    #[error("unsupported XISF layout for update_file: {0}")]
    Unsupported(String),
}
