//! The [`FitsKeyword`] record.

use crate::value::{FromField, IntoValue, Value};

/// A single FITS keyword extracted from (or destined for) an XISF header:
/// a name, a value, and an optional comment.
///
/// The value's on-disk kind (quoted string vs. bare literal) is preserved; see
/// [`IntoValue`] for how the kind is chosen when you write one.
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
    #[must_use]
    pub fn value_str(&self) -> &str {
        self.value.text()
    }

    /// Interpret the value as `T` (see [`FromField`]).
    #[must_use]
    pub fn get<T: FromField>(&self) -> Option<T> {
        T::from_field(self.value.text())
    }
}
