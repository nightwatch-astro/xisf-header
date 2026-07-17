// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Value representation and the [`FromField`] / [`IntoValue`] conversion traits.

use time::PrimitiveDateTime;

/// A keyword value together with how it should be serialized.
///
/// XISF stores FITS keyword values with FITS formatting conventions: string
/// values are single-quoted, everything else (numbers, logicals) is bare. The
/// distinction is preserved so a value round-trips as the same kind it was.
///
/// ```
/// use xisf_header::Value;
///
/// let string_value = Value::Str("Master Dark".to_owned());
/// let literal_value = Value::Literal("300".to_owned());
/// assert_eq!(string_value.text(), "Master Dark");
/// assert_eq!(literal_value.text(), "300");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
    /// A string value (serialized single-quoted, `''`-escaped).
    Str(String),
    /// A bare literal — number, logical, or any pre-formatted token.
    Literal(String),
}

impl Value {
    /// The underlying text, regardless of kind.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Value::Str(s) | Value::Literal(s) => s,
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Str(String::new())
    }
}

/// Interpret a keyword's value text as a Rust type.
///
/// This is the open extension point behind [`Header::get`](crate::Header::get):
/// implement it for your own type to call `header.get::<MyType>(key)`.
/// Returning `None` means "the value cannot be read as this type" (treated as
/// absence, never an error).
///
/// ```
/// use xisf_header::FromField;
///
/// assert_eq!(f64::from_field("300"), Some(300.0));
/// assert_eq!(bool::from_field("T"), Some(true));
/// assert_eq!(i64::from_field("not a number"), None);
/// ```
pub trait FromField: Sized {
    /// Parse the value text into `Self`, or `None` if it does not apply.
    fn from_field(text: &str) -> Option<Self>;
}

impl FromField for String {
    fn from_field(text: &str) -> Option<Self> {
        Some(text.to_owned())
    }
}

impl FromField for f64 {
    fn from_field(text: &str) -> Option<Self> {
        text.trim().parse().ok()
    }
}

impl FromField for i64 {
    fn from_field(text: &str) -> Option<Self> {
        lenient_int(text)
    }
}

impl FromField for u32 {
    fn from_field(text: &str) -> Option<Self> {
        u32::try_from(lenient_int(text)?).ok()
    }
}

impl FromField for bool {
    fn from_field(text: &str) -> Option<Self> {
        match text.trim() {
            "T" | "t" | "1" => Some(true),
            "F" | "f" | "0" => Some(false),
            s if s.eq_ignore_ascii_case("true") => Some(true),
            s if s.eq_ignore_ascii_case("false") => Some(false),
            _ => None,
        }
    }
}

impl FromField for PrimitiveDateTime {
    fn from_field(text: &str) -> Option<Self> {
        parse_datetime(text)
    }
}

/// Lenient integer parse: accepts `20` and the decimal form `20.0`.
fn lenient_int(text: &str) -> Option<i64> {
    let t = text.trim();
    if let Ok(n) = t.parse::<i64>() {
        return Some(n);
    }
    let f = t.parse::<f64>().ok()?;
    if f.fract() == 0.0 && f.is_finite() && f.abs() < 9.007_199_254_740_992e15 {
        // integral f64 within i64's exactly-representable range
        Some(f as i64)
    } else {
        None
    }
}

/// Parse a FITS/XISF ISO-8601 civil date/time, with or without fractional
/// seconds, or a bare calendar date (interpreted at midnight). Delegates to
/// `skymath::parse_date_obs` so this crate carries one implementation of
/// FITS `DATE-OBS` parsing instead of a hand-rolled duplicate (see
/// nightwatch-astro/xisf-header#5).
fn parse_datetime(text: &str) -> Option<PrimitiveDateTime> {
    skymath::parse_date_obs(text).ok()
}

/// Produce a [`Value`] for a write, with the on-disk kind chosen by the Rust
/// type: strings become quoted string values, numbers and logicals become bare
/// literals. Use [`Literal`], [`Fixed`], or [`Sci`] for controlled formatting.
///
/// ```
/// use xisf_header::{IntoValue, Value};
///
/// assert_eq!("Master Dark".into_value(), Value::Str("Master Dark".to_owned()));
/// assert_eq!(300.0.into_value().text(), "300.0");
/// ```
pub trait IntoValue {
    /// Convert `self` into a serializable [`Value`].
    fn into_value(self) -> Value;
}

impl IntoValue for Value {
    fn into_value(self) -> Value {
        self
    }
}

impl IntoValue for &str {
    fn into_value(self) -> Value {
        Value::Str(self.to_owned())
    }
}

impl IntoValue for String {
    fn into_value(self) -> Value {
        Value::Str(self)
    }
}

impl IntoValue for f64 {
    fn into_value(self) -> Value {
        Value::Literal(format_f64(self))
    }
}

impl IntoValue for i64 {
    fn into_value(self) -> Value {
        Value::Literal(self.to_string())
    }
}

impl IntoValue for u32 {
    fn into_value(self) -> Value {
        Value::Literal(self.to_string())
    }
}

impl IntoValue for bool {
    fn into_value(self) -> Value {
        Value::Literal(if self { "T" } else { "F" }.to_owned())
    }
}

/// Write a value as a bare literal exactly as given (escape hatch for
/// pre-formatted or vendor-specific tokens).
///
/// ```
/// use xisf_header::{Header, Literal};
///
/// let mut header = Header::new();
/// header.set("FLAGS", Literal("0x1F".to_owned()))?;
/// assert_eq!(header.get_str("FLAGS")?, Some("0x1F"));
/// # Ok::<(), xisf_header::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct Literal(pub String);

impl IntoValue for Literal {
    fn into_value(self) -> Value {
        Value::Literal(self.0)
    }
}

/// Write a float in fixed-point notation with `decimals` fractional digits.
///
/// ```
/// use xisf_header::{Fixed, Header};
///
/// let mut header = Header::new();
/// header.set("EXPTIME", Fixed(300.0, 2))?;
/// assert_eq!(header.get_str("EXPTIME")?, Some("300.00"));
/// # Ok::<(), xisf_header::Error>(())
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Fixed(pub f64, pub usize);

impl IntoValue for Fixed {
    fn into_value(self) -> Value {
        Value::Literal(format!("{:.*}", self.1, self.0))
    }
}

/// Write a float in scientific notation with `sig_digits` significant digits,
/// using the FITS `E` exponent marker.
///
/// ```
/// use xisf_header::{Header, Sci};
///
/// let mut header = Header::new();
/// header.set("FLUX", Sci(1234.5, 3))?;
/// assert_eq!(header.get_str("FLUX")?, Some("1.23E3"));
/// # Ok::<(), xisf_header::Error>(())
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Sci(pub f64, pub usize);

impl IntoValue for Sci {
    fn into_value(self) -> Value {
        let digits = self.1.saturating_sub(1);
        Value::Literal(format!("{:.*e}", digits, self.0).replace('e', "E"))
    }
}

/// Format an `f64` as the shortest round-trippable decimal, normalized to read
/// as a float (a `.` or exponent is always present).
fn format_f64(v: f64) -> String {
    let s = format!("{v}");
    if s.contains(['.', 'e', 'E']) || !v.is_finite() {
        // already floating-looking, or inf/NaN
        s
    } else {
        format!("{s}.0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_field_string_is_verbatim() {
        assert_eq!(String::from_field(" M31 "), Some(" M31 ".to_owned()));
    }

    #[test]
    fn from_field_f64() {
        assert_eq!(f64::from_field("300"), Some(300.0));
        assert_eq!(f64::from_field(" 3.5 "), Some(3.5));
        assert_eq!(f64::from_field("1e3"), Some(1000.0));
        assert_eq!(f64::from_field("abc"), None);
    }

    #[test]
    fn from_field_i64_is_lenient() {
        assert_eq!(i64::from_field("20"), Some(20));
        assert_eq!(i64::from_field("20.0"), Some(20));
        assert_eq!(i64::from_field(" -7 "), Some(-7));
        assert_eq!(i64::from_field("1e10"), Some(10_000_000_000));
        assert_eq!(i64::from_field("20.5"), None);
        assert_eq!(i64::from_field("1e300"), None); // beyond exact-f64 range
        assert_eq!(i64::from_field("inf"), None);
        assert_eq!(i64::from_field("abc"), None);
    }

    #[test]
    fn from_field_u32() {
        assert_eq!(u32::from_field("2"), Some(2));
        assert_eq!(u32::from_field("2.0"), Some(2));
        assert_eq!(u32::from_field("-1"), None);
        assert_eq!(u32::from_field("4294967296"), None); // u32::MAX + 1
    }

    #[test]
    fn from_field_bool_spellings() {
        for t in ["T", "t", "1", "true", "TRUE", " T "] {
            assert_eq!(bool::from_field(t), Some(true), "{t:?}");
        }
        for f in ["F", "f", "0", "false", "FALSE"] {
            assert_eq!(bool::from_field(f), Some(false), "{f:?}");
        }
        assert_eq!(bool::from_field("yes"), None);
        assert_eq!(bool::from_field(""), None);
    }

    #[test]
    fn from_field_datetime_forms() {
        let dt = PrimitiveDateTime::from_field("2026-07-11T22:15:03").unwrap();
        assert_eq!((dt.year(), dt.hour(), dt.second()), (2026, 22, 3));

        let frac = PrimitiveDateTime::from_field("2026-07-11T22:15:03.25").unwrap();
        assert_eq!(frac.millisecond(), 250);

        // A trailing Z (UTC designator) is tolerated.
        assert!(PrimitiveDateTime::from_field("2026-07-11T22:15:03Z").is_some());

        // A bare calendar date reads as midnight.
        let date = PrimitiveDateTime::from_field("2026-07-11").unwrap();
        assert_eq!((date.hour(), date.minute()), (0, 0));

        assert_eq!(PrimitiveDateTime::from_field("2026-13-40T00:00:00"), None);
        assert_eq!(PrimitiveDateTime::from_field("not a date"), None);
    }

    #[test]
    fn into_value_kind_selection() {
        assert_eq!("s".into_value(), Value::Str("s".to_owned()));
        assert_eq!(String::from("s").into_value(), Value::Str("s".to_owned()));
        assert_eq!(100_i64.into_value(), Value::Literal("100".to_owned()));
        assert_eq!(2_u32.into_value(), Value::Literal("2".to_owned()));
        assert_eq!(true.into_value(), Value::Literal("T".to_owned()));
        assert_eq!(false.into_value(), Value::Literal("F".to_owned()));
        let v = Value::Literal("x".to_owned());
        assert_eq!(v.clone().into_value(), v);
    }

    #[test]
    fn f64_formatting_is_normalized() {
        assert_eq!(300.0.into_value(), Value::Literal("300.0".to_owned()));
        assert_eq!(0.5.into_value(), Value::Literal("0.5".to_owned()));
        // Huge magnitudes render as full decimals; the text must read back
        // as the identical float.
        let huge = 1e300.into_value();
        assert_eq!(f64::from_field(huge.text()), Some(1e300));
        assert!(huge.text().ends_with(".0"));
        assert_eq!(f64::INFINITY.into_value(), Value::Literal("inf".to_owned()));
        assert_eq!(f64::NAN.into_value(), Value::Literal("NaN".to_owned()));
    }

    #[test]
    fn controlled_formatting_wrappers() {
        assert_eq!(
            Fixed(300.0, 2).into_value(),
            Value::Literal("300.00".to_owned())
        );
        assert_eq!(
            Sci(1234.5, 3).into_value(),
            Value::Literal("1.23E3".to_owned())
        );
        assert_eq!(
            Sci(0.00012345, 2).into_value(),
            Value::Literal("1.2E-4".to_owned())
        );
        assert_eq!(
            Literal("0x1F".to_owned()).into_value(),
            Value::Literal("0x1F".to_owned())
        );
    }

    #[test]
    fn value_text_and_default() {
        assert_eq!(Value::Str("a".to_owned()).text(), "a");
        assert_eq!(Value::Literal("1".to_owned()).text(), "1");
        assert_eq!(Value::default(), Value::Str(String::new()));
    }
}
