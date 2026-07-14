#![forbid(unsafe_code)]
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
#![doc = include_str!("../README.md")]

mod error;
mod header;
mod key;
mod keyword;
mod property;
mod reader;
mod splice;
mod value;
mod writer;

pub use error::{Error, Result};
pub use header::{Header, StructuralHints};
pub use key::Key;
pub use keyword::FitsKeyword;
pub use property::Property;
pub use value::{Fixed, FromField, IntoValue, Literal, Sci, Value};

/// Re-export of [`time`], whose types appear in this crate's public API.
pub use time;

#[doc = include_str!("../docs/guide.md")]
pub mod guide {}
