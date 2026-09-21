// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! Safe port of cJSON 1.7.19. This crate forbids `unsafe`.
#![forbid(unsafe_code)]

pub mod compare;
pub mod dom;
pub mod minify;
pub mod number;
pub mod parse;
pub mod print;
pub mod store;
pub mod string;
pub mod types;
pub mod utils;

pub use compare::compare;
pub use minify::minify;
pub use parse::{parse, parse_with_end, ParseError, ParseOpts};
pub use print::{print, print_preallocated, PrintOpts};
pub use store::{NodeId, SafeStore, Store, StrId};
pub use types::{VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH, VERSION_STRING};
