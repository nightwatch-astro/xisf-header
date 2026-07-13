//! The [`Header`] value, [`StructuralHints`], and the keyword/property API.

use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::key::Key;
use crate::keyword::FitsKeyword;
use crate::property::Property;
use crate::value::{FromField, IntoValue};

/// Geometry hints used when serializing a standalone container. A [`Header`]
/// stores only keywords and properties — never image structure — so these
/// hints always supply the `<Image>` element's `geometry`, `sampleFormat`, and
/// `colorSpace`. Defaults to a minimal 1×1 8-bit grayscale image.
///
/// ```
/// use xisf_header::StructuralHints;
///
/// let hints = StructuralHints {
///     geometry: "6248:4176:1".to_owned(),
///     sample_format: "UInt16".to_owned(),
///     color_space: "Gray".to_owned(),
/// };
/// assert_eq!(hints.sample_format, "UInt16");
/// assert_eq!(StructuralHints::default().geometry, "1:1:1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StructuralHints {
    /// XISF `geometry` attribute, e.g. `"1:1:1"` (width:height:channels).
    pub geometry: String,
    /// XISF `sampleFormat`, e.g. `"UInt8"`.
    pub sample_format: String,
    /// XISF `colorSpace`, e.g. `"Gray"`.
    pub color_space: String,
}

impl Default for StructuralHints {
    fn default() -> Self {
        Self {
            geometry: "1:1:1".to_owned(),
            sample_format: "UInt8".to_owned(),
            color_space: "Gray".to_owned(),
        }
    }
}

/// A parsed XISF header: an ordered list of [`FitsKeyword`]s plus a map of
/// XISF `<Property>` elements.
///
/// Keyword access is **strict**: a bare name must be unique, or the accessor
/// returns [`Error::Ambiguous`]. Repeated keywords are reached with an
/// `(name, n)` key or the `get_all`/`count` helpers. Keyword order is
/// preserved; property iteration is ordered by id, not document order.
///
/// This struct is an **in-memory model only**: mutating it (`set`, `append`,
/// `remove`, `set_property`, …) changes nothing on disk. Persist the result
/// with [`write_to_file`](Self::write_to_file) to create a new file, or
/// [`update_file`](Self::update_file) to splice the change into an existing
/// one.
///
/// ```
/// use xisf_header::Header;
///
/// let mut header = Header::new();
/// header.set("IMAGETYP", "Master Dark")?;
/// header.set("EXPTIME", 300.0)?;
/// assert_eq!(header.get_str("IMAGETYP")?, Some("Master Dark"));
/// # Ok::<(), xisf_header::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Header {
    pub(crate) keywords: Vec<FitsKeyword>,
    pub(crate) properties: BTreeMap<String, Property>,
}

impl Header {
    /// Create an empty header.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let header = Header::new();
    /// assert_eq!(header.keywords().len(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ----- keyword reads -------------------------------------------------

    /// Interpret the addressed keyword's value as `T`.
    ///
    /// Returns `Ok(None)` when the keyword is absent or its value cannot be read
    /// as `T`, and [`Error::Ambiguous`] when a bare name matches more than one
    /// keyword.
    ///
    /// # Errors
    ///
    /// [`Error::Ambiguous`] on a duplicated bare name; [`Error::IndexOutOfRange`]
    /// for an `(name, n)` index past the last occurrence.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("OBJECT", "NGC 7000")?;
    /// assert_eq!(header.get::<String>("OBJECT")?, Some("NGC 7000".to_owned()));
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn get<'a, T: FromField>(&self, key: impl Into<Key<'a>>) -> Result<Option<T>> {
        Ok(self
            .resolve(key.into())?
            .and_then(|i| self.keywords[i].get::<T>()))
    }

    /// The addressed keyword's raw value text.
    ///
    /// # Errors
    ///
    /// See [`Header::get`].
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("IMAGETYP", "Master Dark")?;
    /// assert_eq!(header.get_str("IMAGETYP")?, Some("Master Dark"));
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn get_str<'a>(&self, key: impl Into<Key<'a>>) -> Result<Option<&str>> {
        Ok(self
            .resolve(key.into())?
            .map(|i| self.keywords[i].value_str()))
    }

    /// The addressed keyword's value as an `f64`.
    ///
    /// # Errors
    ///
    /// See [`Header::get`].
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("EXPTIME", 300.0)?;
    /// assert_eq!(header.get_f64("EXPTIME")?, Some(300.0));
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn get_f64<'a>(&self, key: impl Into<Key<'a>>) -> Result<Option<f64>> {
        self.get(key)
    }

    /// The addressed keyword's value as an `i64` (accepts `20` and `20.0`).
    ///
    /// # Errors
    ///
    /// See [`Header::get`].
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("GAIN", 100_i64)?;
    /// assert_eq!(header.get_i64("GAIN")?, Some(100));
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn get_i64<'a>(&self, key: impl Into<Key<'a>>) -> Result<Option<i64>> {
        self.get(key)
    }

    /// The addressed keyword's value as a `u32`.
    ///
    /// # Errors
    ///
    /// See [`Header::get`].
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("XBINNING", 2_u32)?;
    /// assert_eq!(header.get_u32("XBINNING")?, Some(2));
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn get_u32<'a>(&self, key: impl Into<Key<'a>>) -> Result<Option<u32>> {
        self.get(key)
    }

    /// The addressed keyword's value as a `bool` (FITS `T`/`F`).
    ///
    /// # Errors
    ///
    /// See [`Header::get`].
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("SIMPLE", true)?;
    /// assert_eq!(header.get_bool("SIMPLE")?, Some(true));
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn get_bool<'a>(&self, key: impl Into<Key<'a>>) -> Result<Option<bool>> {
        self.get(key)
    }

    /// The addressed keyword's value as a civil date/time.
    ///
    /// # Errors
    ///
    /// See [`Header::get`].
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("DATE-OBS", "2026-07-11T22:15:03")?;
    /// let observed = header.get_datetime("DATE-OBS")?.unwrap();
    /// assert_eq!(observed.year(), 2026);
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn get_datetime<'a>(
        &self,
        key: impl Into<Key<'a>>,
    ) -> Result<Option<time::PrimitiveDateTime>> {
        self.get(key)
    }

    /// Every value for `name`, in order, that reads as `T`.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.append("HISTORY", "reduced with siril").unwrap();
    /// header.append("HISTORY", "stacked 20x300s").unwrap();
    /// assert_eq!(
    ///     header.get_all::<String>("HISTORY"),
    ///     vec!["reduced with siril", "stacked 20x300s"]
    /// );
    /// ```
    pub fn get_all<T: FromField>(&self, name: &str) -> Vec<T> {
        self.indices(name)
            .filter_map(|i| self.keywords[i].get::<T>())
            .collect()
    }

    /// How many keywords carry `name` (case-insensitive).
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.append("HISTORY", "reduced with siril").unwrap();
    /// header.append("HISTORY", "stacked 20x300s").unwrap();
    /// assert_eq!(header.count("HISTORY"), 2);
    /// ```
    #[must_use]
    pub fn count(&self, name: &str) -> usize {
        self.indices(name).count()
    }

    /// All keywords in document order.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("GAIN", 100_i64).unwrap();
    /// assert_eq!(header.keywords().len(), 1);
    /// assert_eq!(header.keywords()[0].name, "GAIN");
    /// ```
    #[must_use]
    pub fn keywords(&self) -> &[FitsKeyword] {
        &self.keywords
    }

    /// Iterate the keywords in document order.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("IMAGETYP", "Master Dark").unwrap();
    /// header.set("EXPTIME", 300.0).unwrap();
    /// let names: Vec<&str> = header.iter().map(|k| k.name.as_str()).collect();
    /// assert_eq!(names, ["IMAGETYP", "EXPTIME"]);
    /// ```
    pub fn iter(&self) -> std::slice::Iter<'_, FitsKeyword> {
        self.keywords.iter()
    }

    // ----- keyword writes ------------------------------------------------

    /// Set a keyword's value: update in place when the name is unique, append
    /// when absent. The existing comment is preserved.
    ///
    /// This changes the in-memory header only — nothing is written to disk
    /// until you call [`write_to_file`](Self::write_to_file) (new file) or
    /// [`update_file`](Self::update_file) (existing file).
    ///
    /// For XISF-native structured metadata, see
    /// [`set_property`](Self::set_property).
    ///
    /// # Errors
    ///
    /// [`Error::Ambiguous`] when a bare name is duplicated (use `(name, n)` or
    /// `set_at`-style selection), [`Error::IndexOutOfRange`] for a bad occurrence
    /// index, or [`Error::InvalidName`] when creating an invalid keyword.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("IMAGETYP", "Master Dark")?; // absent: appended
    /// header.set("IMAGETYP", "Master Flat")?; // unique: updated in place
    /// assert_eq!(header.get_str("IMAGETYP")?, Some("Master Flat"));
    /// assert_eq!(header.keywords().len(), 1);
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn set<'a>(&mut self, key: impl Into<Key<'a>>, value: impl IntoValue) -> Result<()> {
        let key = key.into();
        let value = value.into_value();
        match key {
            Key::Name(name) => match self.resolve(Key::Name(name))? {
                Some(i) => self.keywords[i].value = value,
                None => {
                    Self::validate_name(name)?;
                    self.keywords.push(FitsKeyword {
                        name: name.to_owned(),
                        value,
                        comment: String::new(),
                    });
                }
            },
            Key::Nth(name, n) => {
                let i = self.require_nth(name, n)?;
                self.keywords[i].value = value;
            }
        }
        Ok(())
    }

    /// Append a keyword unconditionally (allowing duplicate names). This is how
    /// commentary keywords such as `HISTORY` are built up.
    ///
    /// This changes the in-memory header only — nothing is written to disk
    /// until you call [`write_to_file`](Self::write_to_file) (new file) or
    /// [`update_file`](Self::update_file) (existing file).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidName`] if `name` is not a valid keyword.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.append("HISTORY", "reduced with siril")?;
    /// header.append("HISTORY", "stacked 20x300s")?;
    /// assert_eq!(header.count("HISTORY"), 2);
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn append(&mut self, name: &str, value: impl IntoValue) -> Result<()> {
        Self::validate_name(name)?;
        self.keywords.push(FitsKeyword {
            name: name.to_owned(),
            value: value.into_value(),
            comment: String::new(),
        });
        Ok(())
    }

    /// Set (or clear, with `""`) the comment on the addressed keyword.
    /// Returns `true` if a keyword was found.
    ///
    /// # Errors
    ///
    /// See [`Header::get`]: [`Error::Ambiguous`] on a duplicated bare name,
    /// [`Error::IndexOutOfRange`] for an out-of-range `(name, n)` index.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("IMAGETYP", "Master Dark")?;
    /// assert!(header.set_comment("IMAGETYP", "Type of image")?);
    /// assert_eq!(header.keywords()[0].comment, "Type of image");
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn set_comment<'a>(
        &mut self,
        key: impl Into<Key<'a>>,
        comment: impl Into<String>,
    ) -> Result<bool> {
        match self.resolve(key.into())? {
            Some(i) => {
                self.keywords[i].comment = comment.into();
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Set a keyword's value and comment together.
    ///
    /// # Errors
    ///
    /// See [`Header::set`].
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set_with_comment("GAIN", 100_i64, "Sensor gain")?;
    /// assert_eq!(header.get_i64("GAIN")?, Some(100));
    /// assert_eq!(header.keywords()[0].comment, "Sensor gain");
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn set_with_comment<'a>(
        &mut self,
        key: impl Into<Key<'a>>,
        value: impl IntoValue,
        comment: impl Into<String>,
    ) -> Result<()> {
        let key = key.into();
        self.set(key, value)?;
        if let Some(i) = self.resolve(key)? {
            self.keywords[i].comment = comment.into();
        }
        Ok(())
    }

    /// Remove the addressed keyword. Returns `true` if one was removed.
    ///
    /// This changes the in-memory header only — nothing is written to disk
    /// until you call [`write_to_file`](Self::write_to_file) (new file) or
    /// [`update_file`](Self::update_file) (existing file).
    ///
    /// # Errors
    ///
    /// See [`Header::get`]: [`Error::Ambiguous`] on a duplicated bare name,
    /// [`Error::IndexOutOfRange`] for an out-of-range `(name, n)` index.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set("GAIN", 100_i64)?;
    /// assert!(header.remove("GAIN")?);
    /// assert_eq!(header.get_i64("GAIN")?, None);
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn remove<'a>(&mut self, key: impl Into<Key<'a>>) -> Result<bool> {
        match self.resolve(key.into())? {
            Some(i) => {
                self.keywords.remove(i);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Remove every keyword named `name`. Returns how many were removed.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.append("HISTORY", "reduced with siril").unwrap();
    /// header.append("HISTORY", "stacked 20x300s").unwrap();
    /// assert_eq!(header.remove_all("HISTORY"), 2);
    /// assert_eq!(header.count("HISTORY"), 0);
    /// ```
    pub fn remove_all(&mut self, name: &str) -> usize {
        let before = self.keywords.len();
        self.keywords.retain(|k| !k.name.eq_ignore_ascii_case(name));
        before - self.keywords.len()
    }

    /// Apply several single-keyword upserts atomically: validate every entry
    /// first, then apply all — or, on any rejection, apply none.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidName`] or [`Error::Ambiguous`] for any entry; on error the
    /// header is unchanged.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set_many([("IMAGETYP", "Master Dark"), ("OBJECT", "NGC 7000")])?;
    /// assert_eq!(header.get_str("IMAGETYP")?, Some("Master Dark"));
    /// assert_eq!(header.get_str("OBJECT")?, Some("NGC 7000"));
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn set_many<'a, V, I>(&mut self, entries: I) -> Result<()>
    where
        V: IntoValue,
        I: IntoIterator<Item = (&'a str, V)>,
    {
        let entries: Vec<(&str, V)> = entries.into_iter().collect();
        for (name, _) in &entries {
            Self::validate_name(name)?;
            let count = self.count(name);
            if count > 1 {
                return Err(Error::Ambiguous {
                    name: (*name).to_owned(),
                    count,
                });
            }
        }
        for (name, value) in entries {
            match self.first_index(name) {
                Some(i) => self.keywords[i].value = value.into_value(),
                None => self.keywords.push(FitsKeyword {
                    name: name.to_owned(),
                    value: value.into_value(),
                    comment: String::new(),
                }),
            }
        }
        Ok(())
    }

    /// Remove several keywords atomically. Returns how many were removed.
    ///
    /// # Errors
    ///
    /// [`Error::Ambiguous`] if any name is duplicated; on error the header is
    /// unchanged.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set_many([("IMAGETYP", "Master Dark"), ("OBJECT", "NGC 7000")])?;
    /// assert_eq!(header.remove_many(["IMAGETYP", "OBJECT"])?, 2);
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn remove_many<'a, I: IntoIterator<Item = &'a str>>(&mut self, names: I) -> Result<usize> {
        let names: Vec<&str> = names.into_iter().collect();
        for name in &names {
            let count = self.count(name);
            if count > 1 {
                return Err(Error::Ambiguous {
                    name: (*name).to_owned(),
                    count,
                });
            }
        }
        let mut removed = 0;
        for name in names {
            if let Some(i) = self.first_index(name) {
                self.keywords.remove(i);
                removed += 1;
            }
        }
        Ok(removed)
    }

    // ----- property CRUD -------------------------------------------------

    /// All `<Property>` entries, keyed by `id`. Iteration is ordered by id,
    /// not by document order.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set_property("Observation:Object:Name", "NGC 7000").unwrap();
    /// assert_eq!(header.properties().len(), 1);
    /// ```
    #[must_use]
    pub fn properties(&self) -> &BTreeMap<String, Property> {
        &self.properties
    }

    /// A property's raw value text by `id`.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set_property("Observation:Object:Name", "NGC 7000").unwrap();
    /// assert_eq!(header.property("Observation:Object:Name"), Some("NGC 7000"));
    /// ```
    #[must_use]
    pub fn property(&self, id: &str) -> Option<&str> {
        self.properties.get(id).map(|p| p.value.as_str())
    }

    /// A property value interpreted as `T`.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header
    ///     .set_property_with_type("Instrument:Telescope:FocalLength", "0.53", "Float32")
    ///     .unwrap();
    /// assert_eq!(
    ///     header.property_get::<f64>("Instrument:Telescope:FocalLength"),
    ///     Some(0.53)
    /// );
    /// ```
    #[must_use]
    pub fn property_get<T: FromField>(&self, id: &str) -> Option<T> {
        self.properties
            .get(id)
            .and_then(|p| T::from_field(&p.value))
    }

    /// Insert or update a property's value. An existing property keeps its
    /// `type`, `comment`, and `format`; a new one is created with type
    /// `String`.
    ///
    /// For embedded FITS header keywords, see [`set`](Self::set).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidName`] if `id` is not a valid XISF property id.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set_property("Observation:Object:Name", "NGC 7000")?;
    /// assert_eq!(header.properties()["Observation:Object:Name"].type_, "String");
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn set_property(&mut self, id: impl Into<String>, value: impl Into<String>) -> Result<()> {
        let id = id.into();
        Self::validate_property_id(&id)?;
        self.properties.entry(id).or_default().value = value.into();
        Ok(())
    }

    /// Insert or update a property with an explicit XISF `type` (e.g.
    /// `Float32`, `TimePoint`). An existing property keeps its `comment` and
    /// `format`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidName`] if `id` is not a valid XISF property id.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set_property_with_type("Instrument:Telescope:FocalLength", "0.53", "Float32")?;
    /// assert_eq!(
    ///     header.properties()["Instrument:Telescope:FocalLength"].type_,
    ///     "Float32"
    /// );
    /// # Ok::<(), xisf_header::Error>(())
    /// ```
    pub fn set_property_with_type(
        &mut self,
        id: impl Into<String>,
        value: impl Into<String>,
        type_: impl Into<String>,
    ) -> Result<()> {
        let id = id.into();
        Self::validate_property_id(&id)?;
        let p = self.properties.entry(id).or_default();
        p.value = value.into();
        p.type_ = type_.into();
        Ok(())
    }

    /// Remove a property by `id`. Returns `true` if it existed.
    ///
    /// ```
    /// use xisf_header::Header;
    ///
    /// let mut header = Header::new();
    /// header.set_property("Observation:Object:Name", "NGC 7000").unwrap();
    /// assert!(header.remove_property("Observation:Object:Name"));
    /// assert!(header.property("Observation:Object:Name").is_none());
    /// ```
    pub fn remove_property(&mut self, id: &str) -> bool {
        self.properties.remove(id).is_some()
    }

    // ----- internals -----------------------------------------------------

    fn indices<'s>(&'s self, name: &'s str) -> impl Iterator<Item = usize> + 's {
        self.keywords
            .iter()
            .enumerate()
            .filter(move |(_, k)| k.name.eq_ignore_ascii_case(name))
            .map(|(i, _)| i)
    }

    fn first_index(&self, name: &str) -> Option<usize> {
        self.indices(name).next()
    }

    /// Resolve a key to a keyword index, enforcing the strict rules.
    fn resolve(&self, key: Key) -> Result<Option<usize>> {
        match key {
            Key::Name(name) => {
                let mut it = self.indices(name);
                let first = it.next();
                if first.is_some() && it.next().is_some() {
                    return Err(Error::Ambiguous {
                        name: name.to_owned(),
                        count: self.count(name),
                    });
                }
                Ok(first)
            }
            Key::Nth(name, n) => {
                let indices: Vec<usize> = self.indices(name).collect();
                match indices.get(n) {
                    Some(&i) => Ok(Some(i)),
                    None if indices.is_empty() => Ok(None),
                    None => Err(Error::IndexOutOfRange {
                        name: name.to_owned(),
                        index: n,
                        count: indices.len(),
                    }),
                }
            }
        }
    }

    fn require_nth(&self, name: &str, n: usize) -> Result<usize> {
        self.resolve(Key::Nth(name, n))?
            .ok_or_else(|| Error::IndexOutOfRange {
                name: name.to_owned(),
                index: n,
                count: 0,
            })
    }

    fn validate_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(Error::InvalidName {
                name: name.to_owned(),
                reason: "empty",
            });
        }
        if name.len() > 8 {
            return Err(Error::InvalidName {
                name: name.to_owned(),
                reason: "exceeds 8 characters",
            });
        }
        if !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Error::InvalidName {
                name: name.to_owned(),
                reason: "must be ASCII letters, digits, `-`, or `_`",
            });
        }
        Ok(())
    }

    fn validate_property_id(id: &str) -> Result<()> {
        if id.is_empty() {
            return Err(Error::InvalidName {
                name: id.to_owned(),
                reason: "empty",
            });
        }
        if !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b':')
        {
            return Err(Error::InvalidName {
                name: id.to_owned(),
                reason: "property id must be ASCII alphanumeric, `_`, or `:`",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_name_rules() {
        assert!(Header::validate_name("GAIN").is_ok());
        assert!(Header::validate_name("DATE-OBS").is_ok());
        assert!(Header::validate_name("lower_k").is_ok());
        assert!(Header::validate_name("EIGHTCHR").is_ok());
        assert!(Header::validate_name("").is_err());
        assert!(Header::validate_name("NINECHARS").is_err());
        assert!(Header::validate_name("BAD KEY").is_err());
        assert!(Header::validate_name("NAME!").is_err());
    }

    #[test]
    fn validate_property_id_rules() {
        assert!(Header::validate_property_id("Instrument:Telescope:FocalLength").is_ok());
        assert!(Header::validate_property_id("A_b:9").is_ok());
        assert!(Header::validate_property_id("").is_err());
        assert!(Header::validate_property_id("bad id!").is_err());
        assert!(Header::validate_property_id("hy-phen").is_err());
    }

    #[test]
    fn nth_write_on_absent_name_errors() {
        let mut h = Header::new();
        assert!(matches!(
            h.set(("MISSING", 0), 1_i64),
            Err(Error::IndexOutOfRange { count: 0, .. })
        ));
    }

    #[test]
    fn set_with_comment_creates_and_updates() {
        let mut h = Header::new();
        h.set_with_comment("GAIN", 100_i64, "sensor gain").unwrap();
        assert_eq!(h.get_i64("GAIN").unwrap(), Some(100));
        assert_eq!(h.keywords()[0].comment, "sensor gain");

        h.set_with_comment("GAIN", 200_i64, "updated").unwrap();
        assert_eq!(h.get_i64("GAIN").unwrap(), Some(200));
        assert_eq!(h.keywords()[0].comment, "updated");

        h.append("HISTORY", "a").unwrap();
        h.append("HISTORY", "b").unwrap();
        assert!(matches!(
            h.set_with_comment("HISTORY", "x", "c"),
            Err(Error::Ambiguous { .. })
        ));
    }

    #[test]
    fn set_comment_on_absent_keyword_reports_not_found() {
        let mut h = Header::new();
        assert!(!h.set_comment("MISSING", "c").unwrap());
    }

    #[test]
    fn remove_all_clears_every_occurrence() {
        let mut h = Header::new();
        h.append("HISTORY", "a").unwrap();
        h.append("HISTORY", "b").unwrap();
        h.set("GAIN", 1_i64).unwrap();
        assert_eq!(h.remove_all("history"), 2); // case-insensitive
        assert_eq!(h.count("HISTORY"), 0);
        assert_eq!(h.remove_all("HISTORY"), 0);
        assert_eq!(h.get_i64("GAIN").unwrap(), Some(1));
    }

    #[test]
    fn iter_preserves_document_order() {
        let mut h = Header::new();
        h.set("A", 1_i64).unwrap();
        h.set("B", 2_i64).unwrap();
        h.set("C", 3_i64).unwrap();
        let names: Vec<&str> = h.iter().map(|k| k.name.as_str()).collect();
        assert_eq!(names, ["A", "B", "C"]);
    }

    #[test]
    fn string_keys_are_accepted() {
        let mut h = Header::new();
        let key = String::from("GAIN");
        h.set(&key, 100_i64).unwrap();
        assert_eq!(h.get_i64(&key).unwrap(), Some(100));
    }

    #[test]
    fn generic_get_reads_string() {
        let mut h = Header::new();
        h.set("OBJECT", "M31").unwrap();
        assert_eq!(h.get::<String>("OBJECT").unwrap(), Some("M31".to_owned()));
    }
}
