// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

use std::os::raw::c_char;

use cjson_core::store::Store;
use cjson_core::utils;

use crate::cstore::{cstr_bytes, CStore};
use crate::ffi::CJson;

macro_rules! export {
    ($vis:vis unsafe fn $name:ident($($arg:ident : $t:ty),* $(,)?) $(-> $ret:ty)? { $($body:tt)* }) => {
        #[no_mangle]
        #[cfg(not(windows))]
        $vis unsafe extern "C" fn $name($($arg: $t),*) $(-> $ret)? { $($body)* }
        #[no_mangle]
        #[cfg(windows)]
        $vis unsafe extern "stdcall" fn $name($($arg: $t),*) $(-> $ret)? { $($body)* }
    };
}

fn store() -> CStore {
    CStore
}
fn nid(p: *const CJson) -> Option<cjson_core::NodeId> {
    CStore::node_from_ptr(p as *mut CJson)
}
fn out(id: Option<cjson_core::NodeId>) -> *mut CJson {
    id.map(CStore::ptr_from_node).unwrap_or(std::ptr::null_mut())
}

export! {
    pub unsafe fn cJSONUtils_GetPointer(object: *mut CJson, pointer: *const c_char) -> *mut CJson {
        let st = store();
        out(utils::get_item_from_pointer(&st, nid(object), cstr_bytes(pointer).unwrap_or(b""), false))
    }
}
export! {
    pub unsafe fn cJSONUtils_GetPointerCaseSensitive(object: *mut CJson, pointer: *const c_char) -> *mut CJson {
        let st = store();
        out(utils::get_item_from_pointer(&st, nid(object), cstr_bytes(pointer).unwrap_or(b""), true))
    }
}

export! {
    pub unsafe fn cJSONUtils_GeneratePatches(from: *mut CJson, to: *mut CJson) -> *mut CJson {
        let (Some(f), Some(t)) = (nid(from), nid(to)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(utils::generate_patches(&mut st, f, t, false))
    }
}
export! {
    pub unsafe fn cJSONUtils_GeneratePatchesCaseSensitive(from: *mut CJson, to: *mut CJson) -> *mut CJson {
        let (Some(f), Some(t)) = (nid(from), nid(to)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(utils::generate_patches(&mut st, f, t, true))
    }
}

export! {
    pub unsafe fn cJSONUtils_AddPatchToArray(
        array: *mut CJson,
        operation: *const c_char,
        path: *const c_char,
        value: *const CJson,
    ) {
        let (Some(a), Some(op), Some(p)) = (nid(array), cstr_bytes(operation), cstr_bytes(path)) else { return };
        let mut st = store();
        utils::add_patch_to_array(&mut st, a, op, p, nid(value));
    }
}

export! {
    pub unsafe fn cJSONUtils_ApplyPatches(object: *mut CJson, patches: *const CJson) -> i32 {
        let (Some(o), Some(p)) = (nid(object), nid(patches)) else { return 1 };
        let mut st = store();
        utils::apply_patches(&mut st, o, p, false)
    }
}
export! {
    pub unsafe fn cJSONUtils_ApplyPatchesCaseSensitive(object: *mut CJson, patches: *const CJson) -> i32 {
        let (Some(o), Some(p)) = (nid(object), nid(patches)) else { return 1 };
        let mut st = store();
        utils::apply_patches(&mut st, o, p, true)
    }
}

export! {
    pub unsafe fn cJSONUtils_MergePatch(target: *mut CJson, patch: *const CJson) -> *mut CJson {
        let mut st = store();
        out(utils::merge_patch(&mut st, nid(target), nid(patch), false))
    }
}
export! {
    pub unsafe fn cJSONUtils_MergePatchCaseSensitive(target: *mut CJson, patch: *const CJson) -> *mut CJson {
        let mut st = store();
        out(utils::merge_patch(&mut st, nid(target), nid(patch), true))
    }
}

export! {
    pub unsafe fn cJSONUtils_GenerateMergePatch(from: *mut CJson, to: *mut CJson) -> *mut CJson {
        let mut st = store();
        out(utils::generate_merge_patch(&mut st, nid(from), nid(to), false))
    }
}
export! {
    pub unsafe fn cJSONUtils_GenerateMergePatchCaseSensitive(from: *mut CJson, to: *mut CJson) -> *mut CJson {
        let mut st = store();
        out(utils::generate_merge_patch(&mut st, nid(from), nid(to), true))
    }
}

export! {
    pub unsafe fn cJSONUtils_FindPointerFromObjectTo(object: *const CJson, target: *const CJson) -> *mut c_char {
        let (Some(o), Some(t)) = (nid(object), nid(target)) else { return std::ptr::null_mut() };
        let st = store();
        match utils::find_pointer_from_object_to(&st, o, t) {
            Some(bytes) => {
                // allocate via cJSON_CreateString then steal the pointer? simpler: alloc_str
                let mut st = store();
                match st.alloc_str(&bytes) {
                    Some(s) => CStore::ptr_from_str(s),
                    None => std::ptr::null_mut(),
                }
            }
            None => std::ptr::null_mut(),
        }
    }
}

export! {
    pub unsafe fn cJSONUtils_SortObject(object: *mut CJson) {
        let Some(o) = nid(object) else { return };
        let mut st = store();
        utils::sort_object(&mut st, o, false);
    }
}
export! {
    pub unsafe fn cJSONUtils_SortObjectCaseSensitive(object: *mut CJson) {
        let Some(o) = nid(object) else { return };
        let mut st = store();
        utils::sort_object(&mut st, o, true);
    }
}

