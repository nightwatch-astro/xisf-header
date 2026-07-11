#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod error;
mod header;
mod key;
mod keyword;
mod reader;
mod value;
mod writer;

pub use error::{Error, Result};
pub use header::{Header, StructuralHints};
pub use key::Key;
pub use keyword::FitsKeyword;
pub use value::{Fixed, FromField, IntoValue, Literal, Sci, Value};

/// Re-export of [`time`], whose types appear in this crate's public API.
pub use time;
