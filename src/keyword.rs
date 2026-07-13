//! The [`FitsKeyword`] record.

use crate::value::{FromField, IntoValue, Value};

/// A single FITS keyword extracted from (or destined for) an XISF header:
/// a name, a value, and an optional comment.
///
/// The value's on-disk kind (quoted string vs. bare literal) is preserved; see
/// [`IntoValue`] for how the kind is chosen when you write one.
///
/// ```
/// use xisf_header::FitsKeyword;
///
/// let kw = FitsKeyword::new("IMAGETYP", "Master Dark", "Type of image");
/// assert_eq!(kw.value_str(), "Master Dark");
/// assert_eq!(kw.comment, "Type of image");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FitsKeyword {
    /// The keyword name (e.g. `EXPTIME`), kept verbatim.
    pub name: String,
    /// The keyword value.
    pub(crate) value: Value,
    /// The keyword comment (empty when absent).
    pub comment: String,
}

/// Whether `name` is a FITS commentary keyword (`HISTORY`/`COMMENT`), which
/// carries no FITS value — only free text. XISF represents that text in the
/// `comment` attribute with an empty `value`, unlike every other keyword
/// (spec + reference implementations; see PixInsight-written files). Exact,
/// case-sensitive match: only the canonical uppercase FITS spellings count.
pub(crate) fn is_commentary(name: &str) -> bool {
    matches!(name, "HISTORY" | "COMMENT")
}

impl FitsKeyword {
    /// Create a keyword from its name, value, and comment.
    ///
    /// ```
    /// use xisf_header::FitsKeyword;
    /// let kw = FitsKeyword::new("GAIN", 100_i64, "Sensor gain");
    /// assert_eq!(kw.get::<i64>(), Some(100));
    /// ```
    pub fn new(name: impl Into<String>, value: impl IntoValue, comment: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into_value(),
            comment: comment.into(),
        }
    }

    /// The value's raw text, regardless of kind.
    ///
    /// ```
    /// use xisf_header::FitsKeyword;
    ///
    /// let kw = FitsKeyword::new("OBJECT", "NGC 7000", "Target");
    /// assert_eq!(kw.value_str(), "NGC 7000");
    /// ```
    #[must_use]
    pub fn value_str(&self) -> &str {
        self.value.text()
    }

    /// Interpret the value as `T` (see [`FromField`]).
    ///
    /// ```
    /// use xisf_header::FitsKeyword;
    ///
    /// let kw = FitsKeyword::new("EXPTIME", 300.0, "");
    /// assert_eq!(kw.get::<f64>(), Some(300.0));
    /// ```
    #[must_use]
    pub fn get<T: FromField>(&self) -> Option<T> {
        T::from_field(self.value.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_selects_value_kind_from_rust_type() {
        let s = FitsKeyword::new("OBJECT", "M31", "target");
        assert_eq!(s.value_str(), "M31");
        assert_eq!(s.comment, "target");
        assert_eq!(s.get::<String>(), Some("M31".to_owned()));

        let n = FitsKeyword::new("GAIN", 100_i64, "");
        assert_eq!(n.value_str(), "100");
        assert_eq!(n.get::<i64>(), Some(100));
        assert_eq!(n.get::<bool>(), None);
    }
}
