#![no_std]

extern crate alloc;

mod parser;

pub use parser::{LocaleParseError, LocaleParseOutput, LocaleParser};
