// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

use std::os::raw::c_void;

use crate::ffi::CJsonHooks;

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
}

pub type MallocFn = unsafe extern "C" fn(usize) -> *mut c_void;
pub type FreeFn = unsafe extern "C" fn(*mut c_void);
pub type ReallocFn = unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void;

#[derive(Clone, Copy)]
pub struct Hooks {
    pub allocate: MallocFn,
    pub deallocate: FreeFn,
    pub reallocate: Option<ReallocFn>,
}

impl Hooks {
    pub const fn libc() -> Self {
        Self {
            allocate: malloc,
            deallocate: free,
            reallocate: Some(realloc),
        }
    }
}

static mut GLOBAL_HOOKS: Hooks = Hooks {
    allocate: malloc,
    deallocate: free,
    reallocate: Some(realloc),
};

static mut ERROR_JSON: *const u8 = std::ptr::null();
static mut ERROR_POS: usize = 0;

pub unsafe fn hooks() -> Hooks {
    GLOBAL_HOOKS
}

pub unsafe fn allocate(size: usize) -> *mut c_void {
    (GLOBAL_HOOKS.allocate)(size)
}

pub unsafe fn deallocate(ptr: *mut c_void) {
    (GLOBAL_HOOKS.deallocate)(ptr);
}

pub unsafe fn init_hooks(hooks: *const CJsonHooks) {
    if hooks.is_null() {
        GLOBAL_HOOKS = Hooks::libc();
        return;
    }
    let h = *hooks;
    GLOBAL_HOOKS.allocate = h.malloc_fn.unwrap_or(malloc);
    GLOBAL_HOOKS.deallocate = h.free_fn.unwrap_or(free);
    GLOBAL_HOOKS.reallocate = None;
    if GLOBAL_HOOKS.allocate == malloc && GLOBAL_HOOKS.deallocate == free {
        GLOBAL_HOOKS.reallocate = Some(realloc);
    }
}

pub unsafe fn set_error(json: *const u8, position: usize) {
    ERROR_JSON = json;
    ERROR_POS = position;
}

pub unsafe fn clear_error() {
    ERROR_JSON = std::ptr::null();
    ERROR_POS = 0;
}

pub unsafe fn error_ptr() -> *const u8 {
    ERROR_JSON.wrapping_add(ERROR_POS)
}

pub unsafe fn malloc_user(size: usize) -> *mut c_void {
    allocate(size)
}

pub unsafe fn free_user(ptr: *mut c_void) {
    deallocate(ptr);
}
