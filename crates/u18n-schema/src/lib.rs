#![no_std]

extern crate alloc;

mod parser;

pub use parser::{SchemaParseError, SchemaParseOutput, SchemaParser};
