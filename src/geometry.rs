// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Native XISF `<Image geometry>`: [`ImageGeometry`] and [`GeometryError`].

use thiserror::Error;

/// The well-formed native geometry of an XISF image: its axis lengths and
/// channel count, from the `<Image>` element's `geometry` attribute
/// (`dim1:…:dimN:channel-count`, XISF 1.0 §11.5.1). Every value is positive.
///
/// Read it with [`Header::image_geometry`](crate::Header::image_geometry).
///
/// ```
/// use xisf_header::{Header, StructuralHints};
///
/// let hints = StructuralHints {
///     geometry: "960:540:3".to_owned(),
///     sample_format: "Float32".to_owned(),
///     color_space: "RGB".to_owned(),
/// };
/// let header = Header::parse(&Header::new().to_header_bytes(&hints))?;
/// let Some(Ok(geometry)) = header.image_geometry() else {
///     panic!("expected one well-formed <Image> geometry");
/// };
/// assert_eq!(geometry.dimensions(), &[960, 540]);
/// assert_eq!(geometry.channels(), 3);
/// # Ok::<(), xisf_header::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageGeometry {
    dimensions: Vec<u32>,
    channels: u32,
}

impl ImageGeometry {
    /// The image's length along each axis, in pixels, in declaration order
    /// (X, then Y, then Z, …). Never empty: N = 1 is a one-dimensional
    /// image, N = 2 a two-dimensional one, and so on.
    #[must_use]
    pub fn dimensions(&self) -> &[u32] {
        &self.dimensions
    }

    /// The number of image channels (planes), including any alpha channels.
    #[must_use]
    pub fn channels(&self) -> u32 {
        self.channels
    }

    /// Parse `dim1:…:dimN:channel-count` with N ≥ 1 and every item a
    /// positive `u32`.
    fn parse(text: &str) -> Result<Self, GeometryError> {
        let mut items = Vec::with_capacity(text.matches(':').count() + 1);
        for item in text.split(':') {
            items.push(parse_item(item).ok_or(GeometryError::Malformed)?);
        }
        let channels = items.pop().ok_or(GeometryError::Malformed)?;
        if items.is_empty() {
            return Err(GeometryError::Malformed);
        }
        Ok(Self {
            dimensions: items,
            channels,
        })
    }
}

/// Parse one geometry item: an XISF 1.0 §8.3 plain-text unsigned integer
/// (decimal `\s*[+-]?(0|[1-9][0-9]*)\s*`, or `0b`/`0o`/`0x` binary, octal,
/// or hexadecimal), which must also be positive and fit in a `u32`.
fn parse_item(item: &str) -> Option<u32> {
    let item = item.trim();
    let (digits, radix) = match item.as_bytes() {
        [b'0', b'b' | b'B', ..] => (&item[2..], 2),
        [b'0', b'o' | b'O', ..] => (&item[2..], 8),
        [b'0', b'x' | b'X', ..] => (&item[2..], 16),
        [b'+', ..] => (&item[1..], 10),
        _ => (item, 10),
    };
    // Digits only: no sign, inner whitespace, or empty item reaches
    // `from_str_radix`. A decimal item may not have a leading zero, so
    // `0` and `04` are both rejected here.
    if digits.is_empty()
        || !digits.chars().all(|c| c.is_digit(radix))
        || (radix == 10 && digits.starts_with('0'))
    {
        return None;
    }
    // Fails on overflow past `u32::MAX`.
    let value = u32::from_str_radix(digits, radix).ok()?;
    (value > 0).then_some(value)
}

/// Why a parsed header has no usable [`ImageGeometry`]. Returned inside
/// `Some(Err(_))` by [`Header::image_geometry`](crate::Header::image_geometry);
/// a header with no `<Image>` element returns `None` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
#[non_exhaustive]
pub enum GeometryError {
    /// The header declares more than one `<Image>` element, so no single
    /// image geometry describes the file. None of the images is chosen.
    #[error("XISF header declares more than one <Image> element")]
    MultipleImages,

    /// The single `<Image>` element has no `geometry` attribute, which XISF
    /// requires on every image.
    #[error("XISF <Image> element has no geometry attribute")]
    Missing,

    /// The single `<Image>` element's `geometry` attribute is not
    /// `dim1:…:dimN:channel-count` with N ≥ 1 and every item a positive
    /// `u32`: for example a zero, negative, out-of-range, empty, or
    /// non-integer item, or a channel count with no dimension. Also returned
    /// when the element's attributes are not well-formed XML.
    #[error("malformed XISF <Image> geometry attribute")]
    Malformed,
}

/// The `<Image>` geometry the parser observed in a header. `Default` means
/// no `<Image>` element, which is also the state of every header built in
/// memory.
#[derive(Debug, Clone, Default)]
pub(crate) struct NativeGeometry {
    /// The single `<Image>` element's `geometry` attribute text.
    raw: Option<String>,
    /// `None` until an `<Image>` element has been observed.
    parsed: Option<Result<ImageGeometry, GeometryError>>,
}

impl NativeGeometry {
    /// Record one `<Image>` element in document order. `read_attribute`
    /// returns its `geometry` attribute and runs only for the first image: a
    /// second image leaves no single geometry, whatever either one declares.
    pub(crate) fn observe_image(
        &mut self,
        read_attribute: impl FnOnce() -> Result<Option<String>, GeometryError>,
    ) {
        *self = if self.parsed.is_some() {
            Self {
                raw: None,
                parsed: Some(Err(GeometryError::MultipleImages)),
            }
        } else {
            match read_attribute() {
                Ok(Some(raw)) => Self {
                    parsed: Some(ImageGeometry::parse(&raw)),
                    raw: Some(raw),
                },
                Ok(None) => Self {
                    raw: None,
                    parsed: Some(Err(GeometryError::Missing)),
                },
                Err(e) => Self {
                    raw: None,
                    parsed: Some(Err(e)),
                },
            }
        };
    }

    pub(crate) fn parsed(&self) -> Option<Result<&ImageGeometry, GeometryError>> {
        self.parsed.as_ref().map(|r| r.as_ref().map_err(|e| *e))
    }

    pub(crate) fn raw(&self) -> Option<&str> {
        self.raw.as_deref()
    }
}
