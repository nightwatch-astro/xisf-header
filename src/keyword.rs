//! The [`FitsKeyword`] record and its typed value accessors.

/// A single FITS keyword extracted from (or destined for) an XISF header.
///
/// XISF embeds FITS keywords as `<FITSKeyword name= value= comment=>` elements.
/// The [`value`](Self::value) is stored *unquoted* at rest: any single layer of
/// FITS `'…'` quoting is stripped on parse and re-applied on
/// [`Header::to_bytes`](crate::Header::to_bytes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FitsKeyword {
    /// The keyword name (e.g. `EXPTIME`), kept verbatim.
    pub name: String,
    /// The raw keyword value, with any single FITS quote layer stripped.
    pub value: String,
    /// The keyword comment (empty when absent).
    pub comment: String,
}

impl FitsKeyword {
    /// Create a keyword from its name, value, and comment.
    ///
    /// ```
    /// use xisf_header::FitsKeyword;
    /// let kw = FitsKeyword::new("GAIN", "100", "Sensor gain");
    /// assert_eq!(kw.as_i64(), Some(100));
    /// ```
    pub fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        comment: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            comment: comment.into(),
        }
    }

    /// The value as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// The value parsed as an `i64`, if it parses cleanly.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        self.value.trim().parse().ok()
    }

    /// The value parsed as an `f64`, if it parses cleanly.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        self.value.trim().parse().ok()
    }

    /// The value parsed as a `bool`.
    ///
    /// Accepts the FITS logical literals `T`/`F` as well as the common
    /// `true`/`false` and `1`/`0` spellings (case-insensitively).
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self.value.trim() {
            "T" | "t" | "1" => Some(true),
            "F" | "f" | "0" => Some(false),
            s if s.eq_ignore_ascii_case("true") => Some(true),
            s if s.eq_ignore_ascii_case("false") => Some(false),
            _ => None,
        }
    }
}
