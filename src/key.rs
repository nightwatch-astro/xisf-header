// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The unified [`Key`] used to address keywords.

/// Addresses a keyword either by name (strict: it must be unique) or by a
/// specific occurrence when a name repeats.
///
/// You rarely name `Key` directly — every keyword accessor takes
/// `impl Into<Key>`, so both forms work:
///
/// ```
/// use xisf_header::Header;
/// let mut h = Header::new();
/// h.append("HISTORY", "first").unwrap();
/// h.append("HISTORY", "second").unwrap();
///
/// // Bare name is ambiguous here → error; select an occurrence instead.
/// assert!(h.get_str("HISTORY").is_err());
/// assert_eq!(h.get_str(("HISTORY", 1)).unwrap(), Some("second"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key<'a> {
    /// Match the sole keyword with this name (ambiguous if it repeats).
    Name(&'a str),
    /// Match the `usize`-th (0-based) occurrence of this name.
    Nth(&'a str, usize),
}

impl<'a> Key<'a> {
    /// The keyword name this key addresses.
    ///
    /// ```
    /// use xisf_header::Key;
    ///
    /// let key: Key = ("HISTORY", 1).into();
    /// assert_eq!(key.name(), "HISTORY");
    /// ```
    #[must_use]
    pub fn name(&self) -> &'a str {
        match *self {
            Key::Name(n) | Key::Nth(n, _) => n,
        }
    }
}

impl<'a> From<&'a str> for Key<'a> {
    fn from(name: &'a str) -> Self {
        Key::Name(name)
    }
}

impl<'a> From<&'a String> for Key<'a> {
    fn from(name: &'a String) -> Self {
        Key::Name(name)
    }
}

impl<'a> From<(&'a str, usize)> for Key<'a> {
    fn from((name, n): (&'a str, usize)) -> Self {
        Key::Nth(name, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_and_name() {
        let by_name: Key = "GAIN".into();
        assert_eq!(by_name, Key::Name("GAIN"));
        assert_eq!(by_name.name(), "GAIN");

        let owned = String::from("GAIN");
        let from_string: Key = (&owned).into();
        assert_eq!(from_string, Key::Name("GAIN"));

        let nth: Key = ("HISTORY", 2).into();
        assert_eq!(nth, Key::Nth("HISTORY", 2));
        assert_eq!(nth.name(), "HISTORY");
    }
}
