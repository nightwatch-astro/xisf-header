//! The [`Header`] value: FITS-keyword and `<Property>` CRUD.

use std::collections::BTreeMap;

use crate::keyword::FitsKeyword;

/// A parsed XISF header: an ordered list of [`FitsKeyword`]s plus a map of
/// XISF `<Property>` elements.
///
/// Keyword lookups are **case-insensitive** on the name; keyword *order* is
/// preserved (FITS allows repeated keywords such as `COMMENT`/`HISTORY`).
/// Properties are keyed by their `id` and kept in sorted order for stable
/// serialization.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Header {
    pub(crate) keywords: Vec<FitsKeyword>,
    pub(crate) properties: BTreeMap<String, String>,
}

impl Header {
    /// Create an empty header.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ----- keyword reads -------------------------------------------------

    /// All keywords in document order.
    #[must_use]
    pub fn keywords(&self) -> &[FitsKeyword] {
        &self.keywords
    }

    /// The first keyword with the given name (case-insensitive), if any.
    #[must_use]
    pub fn keyword(&self, name: &str) -> Option<&FitsKeyword> {
        self.keywords
            .iter()
            .find(|k| k.name.eq_ignore_ascii_case(name))
    }

    /// Every keyword with the given name (case-insensitive), in order.
    pub fn get_all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a FitsKeyword> {
        self.keywords
            .iter()
            .filter(move |k| k.name.eq_ignore_ascii_case(name))
    }

    /// The first matching keyword's raw value.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.keyword(name).map(FitsKeyword::as_str)
    }

    /// The first matching keyword's value as a string slice.
    #[must_use]
    pub fn get_str(&self, name: &str) -> Option<&str> {
        self.get(name)
    }

    /// The first matching keyword's value parsed as an `i64`.
    #[must_use]
    pub fn get_i64(&self, name: &str) -> Option<i64> {
        self.keyword(name).and_then(FitsKeyword::as_i64)
    }

    /// The first matching keyword's value parsed as an `f64`.
    #[must_use]
    pub fn get_f64(&self, name: &str) -> Option<f64> {
        self.keyword(name).and_then(FitsKeyword::as_f64)
    }

    /// The first matching keyword's value parsed as a `bool`.
    #[must_use]
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.keyword(name).and_then(FitsKeyword::as_bool)
    }

    // ----- keyword writes ------------------------------------------------

    /// Upsert a keyword: update the first case-insensitive match in place, or
    /// insert a new keyword if none exists.
    ///
    /// ```
    /// use xisf_header::Header;
    /// let mut h = Header::new();
    /// h.set("IMAGETYP", "Master Dark", "Type of image");
    /// h.set("IMAGETYP", "Light", ""); // updates the existing keyword
    /// assert_eq!(h.get_str("imagetyp"), Some("Light"));
    /// ```
    pub fn set(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        comment: impl Into<String>,
    ) {
        let name = name.into();
        let value = value.into();
        let comment = comment.into();
        if let Some(existing) = self
            .keywords
            .iter_mut()
            .find(|k| k.name.eq_ignore_ascii_case(&name))
        {
            existing.value = value;
            existing.comment = comment;
        } else {
            self.keywords.push(FitsKeyword::new(name, value, comment));
        }
    }

    /// Append a keyword unconditionally (allowing duplicate names).
    pub fn push(&mut self, keyword: FitsKeyword) {
        self.keywords.push(keyword);
    }

    /// Append many keywords unconditionally.
    pub fn extend<I: IntoIterator<Item = FitsKeyword>>(&mut self, keywords: I) {
        self.keywords.extend(keywords);
    }

    /// Remove the first keyword with the given name (case-insensitive).
    ///
    /// Returns `true` if a keyword was removed.
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(idx) = self
            .keywords
            .iter()
            .position(|k| k.name.eq_ignore_ascii_case(name))
        {
            self.keywords.remove(idx);
            true
        } else {
            false
        }
    }

    /// Remove every keyword with the given name (case-insensitive).
    ///
    /// Returns the number of keywords removed.
    pub fn remove_all(&mut self, name: &str) -> usize {
        let before = self.keywords.len();
        self.keywords.retain(|k| !k.name.eq_ignore_ascii_case(name));
        before - self.keywords.len()
    }

    // ----- property CRUD -------------------------------------------------

    /// All `<Property>` entries, keyed by `id`.
    #[must_use]
    pub fn properties(&self) -> &BTreeMap<String, String> {
        &self.properties
    }

    /// A property value by `id` (exact match).
    #[must_use]
    pub fn property(&self, id: &str) -> Option<&str> {
        self.properties.get(id).map(String::as_str)
    }

    /// A property value parsed as an `i64`.
    #[must_use]
    pub fn property_i64(&self, id: &str) -> Option<i64> {
        self.property(id).and_then(|v| v.trim().parse().ok())
    }

    /// A property value parsed as an `f64`.
    #[must_use]
    pub fn property_f64(&self, id: &str) -> Option<f64> {
        self.property(id).and_then(|v| v.trim().parse().ok())
    }

    /// Insert or update a property.
    pub fn set_property(&mut self, id: impl Into<String>, value: impl Into<String>) {
        self.properties.insert(id.into(), value.into());
    }

    /// Remove a property by `id`. Returns `true` if it existed.
    pub fn remove_property(&mut self, id: &str) -> bool {
        self.properties.remove(id).is_some()
    }
}
