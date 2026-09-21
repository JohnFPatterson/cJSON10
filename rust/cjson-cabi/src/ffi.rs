// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! `#[repr(C)]` layout matching `cJSON.h`.

use std::os::raw::{c_char, c_double, c_int};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CJson {
    pub next: *mut CJson,
    pub prev: *mut CJson,
    pub child: *mut CJson,
    pub type_: c_int,
    pub valuestring: *mut c_char,
    pub valueint: c_int,
    pub valuedouble: c_double,
    pub string: *mut c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CJsonHooks {
    pub malloc_fn: Option<unsafe extern "C" fn(usize) -> *mut std::os::raw::c_void>,
    pub free_fn: Option<unsafe extern "C" fn(*mut std::os::raw::c_void)>,
}

pub type CJsonBool = c_int;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn lp64_layout() {
        if size_of::<usize>() != 8 {
            return;
        }
        assert_eq!(offset_of!(CJson, next), 0);
        assert_eq!(offset_of!(CJson, prev), 8);
        assert_eq!(offset_of!(CJson, child), 16);
        assert_eq!(offset_of!(CJson, type_), 24);
        assert_eq!(offset_of!(CJson, valuestring), 32);
        assert_eq!(offset_of!(CJson, valueint), 40);
        assert_eq!(offset_of!(CJson, valuedouble), 48);
        assert_eq!(offset_of!(CJson, string), 56);
        assert_eq!(size_of::<CJson>(), 64);
        assert_eq!(align_of::<CJson>(), 8);
    }
}
