#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod error;
mod header;
mod keyword;
mod reader;
mod writer;

pub use error::{Error, Result};
pub use header::Header;
pub use keyword::FitsKeyword;
