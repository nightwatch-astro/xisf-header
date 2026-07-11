//! The [`Property`] record for XISF `<Property>` elements.

/// A single XISF `<Property>`: its `type`, value text, and the optional
/// `comment` and `format` attributes, all preserved verbatim so a property
/// round-trips unchanged.
///
/// Unlike FITS keywords, XISF property values are *not* FITS-formatted: they
/// are stored raw, without any quote layer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Property {
    /// The XISF `type` attribute (e.g. `String`, `Float32`, `TimePoint`),
    /// kept verbatim.
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub type_: String,
    /// The raw value text (from the `value` attribute, or the element's child
    /// text for the long `String` form).
    pub value: String,
    /// The `comment` attribute (empty when absent).
    pub comment: String,
    /// The `format` attribute (empty when absent).
    pub format: String,
}

impl Default for Property {
    fn default() -> Self {
        Self {
            type_: "String".to_owned(),
            value: String::new(),
            comment: String::new(),
            format: String::new(),
        }
    }
}

impl Property {
    /// Create a property of the given XISF type with a raw value.
    ///
    /// ```
    /// use xisf_header::Property;
    /// let p = Property::new("Float32", "0.135");
    /// assert_eq!(p.type_, "Float32");
    /// assert_eq!(p.value, "0.135");
    /// ```
    pub fn new(type_: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            type_: type_.into(),
            value: value.into(),
            ..Self::default()
        }
    }
}
