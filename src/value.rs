//! Value representation and the [`FromField`] / [`IntoValue`] conversion traits.

use time::PrimitiveDateTime;

/// A keyword value together with how it should be serialized.
///
/// XISF stores FITS keyword values with FITS formatting conventions: string
/// values are single-quoted, everything else (numbers, logicals) is bare. The
/// distinction is preserved so a value round-trips as the same kind it was.
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
/// seconds, or a bare calendar date (interpreted at midnight).
fn parse_datetime(text: &str) -> Option<PrimitiveDateTime> {
    let s = text.trim().trim_end_matches('Z');
    for pat in [
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond]",
        "[year]-[month]-[day]T[hour]:[minute]:[second]",
    ] {
        if let Ok(fmt) = time::format_description::parse_borrowed::<2>(pat) {
            if let Ok(dt) = PrimitiveDateTime::parse(s, &fmt) {
                return Some(dt);
            }
        }
    }
    if let Ok(fmt) = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]") {
        if let Ok(date) = time::Date::parse(s, &fmt) {
            return Some(PrimitiveDateTime::new(date, time::Time::MIDNIGHT));
        }
    }
    None
}

/// Produce a [`Value`] for a write, with the on-disk kind chosen by the Rust
/// type: strings become quoted string values, numbers and logicals become bare
/// literals. Use [`Literal`], [`Fixed`], or [`Sci`] for controlled formatting.
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
#[derive(Debug, Clone)]
pub struct Literal(pub String);

impl IntoValue for Literal {
    fn into_value(self) -> Value {
        Value::Literal(self.0)
    }
}

/// Write a float in fixed-point notation with `decimals` fractional digits.
#[derive(Debug, Clone, Copy)]
pub struct Fixed(pub f64, pub usize);

impl IntoValue for Fixed {
    fn into_value(self) -> Value {
        Value::Literal(format!("{:.*}", self.1, self.0))
    }
}

/// Write a float in scientific notation with `sig_digits` significant digits,
/// using the FITS `E` exponent marker.
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
    if s.contains(['.', 'e', 'E', 'n', 'i']) {
        // already floating-looking, or inf/nan
        s
    } else {
        format!("{s}.0")
    }
}
