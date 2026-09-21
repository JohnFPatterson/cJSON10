// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! C ABI shim. This is the only crate allowed to use `unsafe`.

pub mod api;
pub mod cstore;
pub mod ffi;
pub mod hooks;
pub mod utils_api;
