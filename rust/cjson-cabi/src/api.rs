// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

use std::os::raw::{c_char, c_double, c_int, c_void};

use cjson_core::dom;
use cjson_core::parse::{parse_with_end, ParseOpts};
use cjson_core::print::{print, print_preallocated, PrintOpts};
use cjson_core::store::Store;
use cjson_core::types;

use crate::cstore::{cstr_bytes, CStore};
use crate::ffi::{CJson, CJsonBool, CJsonHooks};
use crate::hooks;

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

fn out(id: Option<cjson_core::NodeId>) -> *mut CJson {
    id.map(CStore::ptr_from_node).unwrap_or(std::ptr::null_mut())
}

fn nid(p: *const CJson) -> Option<cjson_core::NodeId> {
    CStore::node_from_ptr(p as *mut CJson)
}

fn bytes_to_cstring_ptr(bytes: &[u8]) -> *mut c_char {
    let mut st = store();
    match st.alloc_str(bytes) {
        Some(s) => CStore::ptr_from_str(s),
        None => std::ptr::null_mut(),
    }
}

export! {
    pub unsafe fn cJSON_Version() -> *const c_char {
        static VER: &[u8] = b"1.7.19\0";
        VER.as_ptr() as *const c_char
    }
}

export! {
    pub unsafe fn cJSON_InitHooks(hooks_ptr: *mut CJsonHooks) {
        hooks::init_hooks(hooks_ptr);
    }
}

fn current_decimal_point() -> u8 {
    #[cfg(feature = "locales")]
    unsafe {
        #[repr(C)]
        struct Lconv {
            decimal_point: *mut c_char,
        }
        extern "C" {
            fn localeconv() -> *mut Lconv;
        }
        let lc = localeconv();
        if lc.is_null() || (*lc).decimal_point.is_null() {
            b'.'
        } else {
            *(*lc).decimal_point as u8
        }
    }
    #[cfg(not(feature = "locales"))]
    {
        b'.'
    }
}

unsafe fn parse_length_opts(
    value: *const c_char,
    buffer_length: usize,
    return_parse_end: *mut *const c_char,
    require_null_terminated: CJsonBool,
) -> *mut CJson {
    hooks::clear_error();
    if value.is_null() || buffer_length == 0 {
        if !value.is_null() {
            hooks::set_error(value as *const u8, 0);
            if !return_parse_end.is_null() {
                *return_parse_end = value;
            }
        }
        return std::ptr::null_mut();
    }
    let input = std::slice::from_raw_parts(value as *const u8, buffer_length);
    let opts = ParseOpts {
        require_null_terminated: require_null_terminated != 0,
        decimal_point: current_decimal_point(),
    };
    let mut st = store();
    match parse_with_end(&mut st, input, opts) {
        Ok((id, end)) => {
            if !return_parse_end.is_null() {
                *return_parse_end = value.add(end);
            }
            CStore::ptr_from_node(id)
        }
        Err(e) => {
            hooks::set_error(value as *const u8, e.offset);
            if !return_parse_end.is_null() {
                *return_parse_end = value.add(e.offset);
            }
            std::ptr::null_mut()
        }
    }
}

export! {
    pub unsafe fn cJSON_ParseWithLengthOpts(
        value: *const c_char,
        buffer_length: usize,
        return_parse_end: *mut *const c_char,
        require_null_terminated: CJsonBool,
    ) -> *mut CJson {
        parse_length_opts(value, buffer_length, return_parse_end, require_null_terminated)
    }
}

export! {
    pub unsafe fn cJSON_ParseWithOpts(
        value: *const c_char,
        return_parse_end: *mut *const c_char,
        require_null_terminated: CJsonBool,
    ) -> *mut CJson {
        if value.is_null() {
            hooks::clear_error();
            return std::ptr::null_mut();
        }
        let len = libc_strlen(value) + 1;
        parse_length_opts(value, len, return_parse_end, require_null_terminated)
    }
}

unsafe fn libc_strlen(s: *const c_char) -> usize {
    let mut n = 0usize;
    while *s.add(n) != 0 {
        n += 1;
    }
    n
}

export! {
    pub unsafe fn cJSON_Parse(value: *const c_char) -> *mut CJson {
        cJSON_ParseWithOpts(value, std::ptr::null_mut(), 0)
    }
}

export! {
    pub unsafe fn cJSON_ParseWithLength(value: *const c_char, buffer_length: usize) -> *mut CJson {
        parse_length_opts(value, buffer_length, std::ptr::null_mut(), 0)
    }
}

export! {
    pub unsafe fn cJSON_Print(item: *const CJson) -> *mut c_char {
        print_item(item, true)
    }
}

export! {
    pub unsafe fn cJSON_PrintUnformatted(item: *const CJson) -> *mut c_char {
        print_item(item, false)
    }
}

unsafe fn print_item(item: *const CJson, format: bool) -> *mut c_char {
    let Some(id) = nid(item) else {
        return std::ptr::null_mut();
    };
    let st = store();
    match print(&st, id, PrintOpts { format }) {
        Some(bytes) => bytes_to_cstring_ptr(&bytes),
        None => std::ptr::null_mut(),
    }
}

export! {
    pub unsafe fn cJSON_PrintBuffered(item: *const CJson, prebuffer: c_int, fmt: CJsonBool) -> *mut c_char {
        if prebuffer < 0 {
            return std::ptr::null_mut();
        }
        let _ = prebuffer;
        print_item(item, fmt != 0)
    }
}

export! {
    pub unsafe fn cJSON_PrintPreallocated(
        item: *mut CJson,
        buffer: *mut c_char,
        length: c_int,
        format: CJsonBool,
    ) -> CJsonBool {
        if length < 0 || buffer.is_null() {
            return 0;
        }
        let Some(id) = nid(item) else {
            return 0;
        };
        let dest = std::slice::from_raw_parts_mut(buffer as *mut u8, length as usize);
        let st = store();
        if print_preallocated(&st, id, dest, format != 0) {
            1
        } else {
            0
        }
    }
}

export! {
    pub unsafe fn cJSON_Delete(item: *mut CJson) {
        let mut st = store();
        st.delete(nid(item));
    }
}

export! {
    pub unsafe fn cJSON_GetArraySize(array: *const CJson) -> c_int {
        let st = store();
        dom::get_array_size(&st, nid(array))
    }
}

export! {
    pub unsafe fn cJSON_GetArrayItem(array: *const CJson, index: c_int) -> *mut CJson {
        let st = store();
        out(dom::get_array_item(&st, nid(array), index))
    }
}

export! {
    pub unsafe fn cJSON_GetObjectItem(object: *const CJson, string: *const c_char) -> *mut CJson {
        let st = store();
        out(dom::get_object_item(&st, nid(object), cstr_bytes(string), false))
    }
}

export! {
    pub unsafe fn cJSON_GetObjectItemCaseSensitive(object: *const CJson, string: *const c_char) -> *mut CJson {
        let st = store();
        out(dom::get_object_item(&st, nid(object), cstr_bytes(string), true))
    }
}

export! {
    pub unsafe fn cJSON_HasObjectItem(object: *const CJson, string: *const c_char) -> CJsonBool {
        if cJSON_GetObjectItem(object, string).is_null() { 0 } else { 1 }
    }
}

export! {
    pub unsafe fn cJSON_GetErrorPtr() -> *const c_char {
        hooks::error_ptr() as *const c_char
    }
}

export! {
    pub unsafe fn cJSON_GetStringValue(item: *const CJson) -> *mut c_char {
        if cJSON_IsString(item) == 0 {
            return std::ptr::null_mut();
        }
        (*item).valuestring
    }
}

export! {
    pub unsafe fn cJSON_GetNumberValue(item: *const CJson) -> c_double {
        if cJSON_IsNumber(item) == 0 {
            return f64::NAN;
        }
        (*item).valuedouble
    }
}

macro_rules! is_type_fn {
    ($name:ident, $pred:path) => {
        export! {
            pub unsafe fn $name(item: *const CJson) -> CJsonBool {
                let st = store();
                if $pred(&st, nid(item)) { 1 } else { 0 }
            }
        }
    };
}

is_type_fn!(cJSON_IsInvalid, dom::is_invalid);
is_type_fn!(cJSON_IsFalse, dom::is_false);
is_type_fn!(cJSON_IsTrue, dom::is_true);
is_type_fn!(cJSON_IsBool, dom::is_bool);
is_type_fn!(cJSON_IsNull, dom::is_null);
is_type_fn!(cJSON_IsNumber, dom::is_number);
is_type_fn!(cJSON_IsString, dom::is_string);
is_type_fn!(cJSON_IsArray, dom::is_array);
is_type_fn!(cJSON_IsObject, dom::is_object);
is_type_fn!(cJSON_IsRaw, dom::is_raw);

export! {
    pub unsafe fn cJSON_CreateNull() -> *mut CJson {
        let mut st = store();
        out(dom::create_null(&mut st))
    }
}
export! {
    pub unsafe fn cJSON_CreateTrue() -> *mut CJson {
        let mut st = store();
        out(dom::create_true(&mut st))
    }
}
export! {
    pub unsafe fn cJSON_CreateFalse() -> *mut CJson {
        let mut st = store();
        out(dom::create_false(&mut st))
    }
}
export! {
    pub unsafe fn cJSON_CreateBool(boolean: CJsonBool) -> *mut CJson {
        let mut st = store();
        out(dom::create_bool(&mut st, boolean != 0))
    }
}
export! {
    pub unsafe fn cJSON_CreateNumber(num: c_double) -> *mut CJson {
        let mut st = store();
        out(dom::create_number(&mut st, num))
    }
}
export! {
    pub unsafe fn cJSON_CreateString(string: *const c_char) -> *mut CJson {
        let mut st = store();
        out(cstr_bytes(string).and_then(|b| dom::create_string(&mut st, b)))
    }
}
export! {
    pub unsafe fn cJSON_CreateRaw(raw: *const c_char) -> *mut CJson {
        let mut st = store();
        out(cstr_bytes(raw).and_then(|b| dom::create_raw(&mut st, b)))
    }
}
export! {
    pub unsafe fn cJSON_CreateArray() -> *mut CJson {
        let mut st = store();
        out(dom::create_array(&mut st))
    }
}
export! {
    pub unsafe fn cJSON_CreateObject() -> *mut CJson {
        let mut st = store();
        out(dom::create_object(&mut st))
    }
}
export! {
    pub unsafe fn cJSON_CreateStringReference(string: *const c_char) -> *mut CJson {
        let mut st = store();
        if string.is_null() {
            let n = st.alloc_node();
            if let Some(id) = n {
                st.set_ty(id, types::STRING | types::IS_REFERENCE);
            }
            return out(n);
        }
        out(cstr_bytes(string).and_then(|b| dom::create_string_reference(&mut st, b)))
    }
}
export! {
    pub unsafe fn cJSON_CreateObjectReference(child: *const CJson) -> *mut CJson {
        let mut st = store();
        out(dom::create_object_reference(&mut st, nid(child)))
    }
}
export! {
    pub unsafe fn cJSON_CreateArrayReference(child: *const CJson) -> *mut CJson {
        let mut st = store();
        out(dom::create_array_reference(&mut st, nid(child)))
    }
}

export! {
    pub unsafe fn cJSON_CreateIntArray(numbers: *const c_int, count: c_int) -> *mut CJson {
        if count < 0 || numbers.is_null() {
            return std::ptr::null_mut();
        }
        let slice = std::slice::from_raw_parts(numbers, count as usize);
        let mut st = store();
        out(dom::create_int_array(&mut st, slice))
    }
}
export! {
    pub unsafe fn cJSON_CreateFloatArray(numbers: *const f32, count: c_int) -> *mut CJson {
        if count < 0 || numbers.is_null() {
            return std::ptr::null_mut();
        }
        let slice = std::slice::from_raw_parts(numbers, count as usize);
        let vals: Vec<f64> = slice.iter().map(|&x| x as f64).collect();
        let mut st = store();
        out(dom::create_double_array(&mut st, &vals))
    }
}
export! {
    pub unsafe fn cJSON_CreateDoubleArray(numbers: *const c_double, count: c_int) -> *mut CJson {
        if count < 0 || numbers.is_null() {
            return std::ptr::null_mut();
        }
        let slice = std::slice::from_raw_parts(numbers, count as usize);
        let mut st = store();
        out(dom::create_double_array(&mut st, slice))
    }
}
export! {
    pub unsafe fn cJSON_CreateStringArray(strings: *const *const c_char, count: c_int) -> *mut CJson {
        if count < 0 || strings.is_null() {
            return std::ptr::null_mut();
        }
        let slice = std::slice::from_raw_parts(strings, count as usize);
        let owned: Vec<Vec<u8>> = slice.iter().filter_map(|p| cstr_bytes(*p).map(|b| b.to_vec())).collect();
        if owned.len() != count as usize {
            return std::ptr::null_mut();
        }
        let refs: Vec<&[u8]> = owned.iter().map(|v| v.as_slice()).collect();
        let mut st = store();
        out(dom::create_string_array(&mut st, &refs))
    }
}

export! {
    pub unsafe fn cJSON_AddItemToArray(array: *mut CJson, item: *mut CJson) -> CJsonBool {
        let (Some(a), Some(i)) = (nid(array), nid(item)) else { return 0 };
        let mut st = store();
        if dom::add_item_to_array(&mut st, a, i) { 1 } else { 0 }
    }
}
export! {
    pub unsafe fn cJSON_AddItemToObject(object: *mut CJson, string: *const c_char, item: *mut CJson) -> CJsonBool {
        let (Some(o), Some(i), Some(k)) = (nid(object), nid(item), cstr_bytes(string)) else { return 0 };
        let mut st = store();
        if dom::add_item_to_object(&mut st, o, k, i, false) { 1 } else { 0 }
    }
}
export! {
    pub unsafe fn cJSON_AddItemToObjectCS(object: *mut CJson, string: *const c_char, item: *mut CJson) -> CJsonBool {
        let (Some(o), Some(i), Some(k)) = (nid(object), nid(item), cstr_bytes(string)) else { return 0 };
        let mut st = store();
        if dom::add_item_to_object(&mut st, o, k, i, true) { 1 } else { 0 }
    }
}
export! {
    pub unsafe fn cJSON_AddItemReferenceToArray(array: *mut CJson, item: *mut CJson) -> CJsonBool {
        let (Some(a), Some(i)) = (nid(array), nid(item)) else { return 0 };
        let mut st = store();
        let Some(r) = dom::create_reference(&mut st, i) else { return 0 };
        if dom::add_item_to_array(&mut st, a, r) { 1 } else { 0 }
    }
}
export! {
    pub unsafe fn cJSON_AddItemReferenceToObject(object: *mut CJson, string: *const c_char, item: *mut CJson) -> CJsonBool {
        let (Some(o), Some(i), Some(k)) = (nid(object), nid(item), cstr_bytes(string)) else { return 0 };
        let mut st = store();
        let Some(r) = dom::create_reference(&mut st, i) else { return 0 };
        if dom::add_item_to_object(&mut st, o, k, r, false) { 1 } else { 0 }
    }
}

export! {
    pub unsafe fn cJSON_DetachItemViaPointer(parent: *mut CJson, item: *mut CJson) -> *mut CJson {
        let (Some(p), Some(i)) = (nid(parent), nid(item)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::detach_item_via_pointer(&mut st, p, i))
    }
}
export! {
    pub unsafe fn cJSON_DetachItemFromArray(array: *mut CJson, which: c_int) -> *mut CJson {
        let Some(a) = nid(array) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::detach_item_from_array(&mut st, a, which))
    }
}
export! {
    pub unsafe fn cJSON_DeleteItemFromArray(array: *mut CJson, which: c_int) {
        let d = cJSON_DetachItemFromArray(array, which);
        cJSON_Delete(d);
    }
}
export! {
    pub unsafe fn cJSON_DetachItemFromObject(object: *mut CJson, string: *const c_char) -> *mut CJson {
        let (Some(o), Some(k)) = (nid(object), cstr_bytes(string)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::detach_item_from_object(&mut st, o, k, false))
    }
}
export! {
    pub unsafe fn cJSON_DetachItemFromObjectCaseSensitive(object: *mut CJson, string: *const c_char) -> *mut CJson {
        let (Some(o), Some(k)) = (nid(object), cstr_bytes(string)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::detach_item_from_object(&mut st, o, k, true))
    }
}
export! {
    pub unsafe fn cJSON_DeleteItemFromObject(object: *mut CJson, string: *const c_char) {
        cJSON_Delete(cJSON_DetachItemFromObject(object, string));
    }
}
export! {
    pub unsafe fn cJSON_DeleteItemFromObjectCaseSensitive(object: *mut CJson, string: *const c_char) {
        cJSON_Delete(cJSON_DetachItemFromObjectCaseSensitive(object, string));
    }
}

export! {
    pub unsafe fn cJSON_InsertItemInArray(array: *mut CJson, which: c_int, newitem: *mut CJson) -> CJsonBool {
        let (Some(a), Some(n)) = (nid(array), nid(newitem)) else { return 0 };
        let mut st = store();
        if dom::insert_item_in_array(&mut st, a, which, n) { 1 } else { 0 }
    }
}
export! {
    pub unsafe fn cJSON_ReplaceItemViaPointer(parent: *mut CJson, item: *mut CJson, replacement: *mut CJson) -> CJsonBool {
        let (Some(p), Some(i), Some(r)) = (nid(parent), nid(item), nid(replacement)) else { return 0 };
        let mut st = store();
        if dom::replace_item_via_pointer(&mut st, p, i, r) { 1 } else { 0 }
    }
}
export! {
    pub unsafe fn cJSON_ReplaceItemInArray(array: *mut CJson, which: c_int, newitem: *mut CJson) -> CJsonBool {
        let Some(item) = nid(cJSON_GetArrayItem(array, which)) else { return 0 };
        cJSON_ReplaceItemViaPointer(array, CStore::ptr_from_node(item), newitem)
    }
}
export! {
    pub unsafe fn cJSON_ReplaceItemInObject(object: *mut CJson, string: *const c_char, newitem: *mut CJson) -> CJsonBool {
        let (Some(o), Some(k), Some(n)) = (nid(object), cstr_bytes(string), nid(newitem)) else { return 0 };
        let mut st = store();
        if dom::replace_item_in_object(&mut st, o, k, n, false) { 1 } else { 0 }
    }
}
export! {
    pub unsafe fn cJSON_ReplaceItemInObjectCaseSensitive(object: *mut CJson, string: *const c_char, newitem: *mut CJson) -> CJsonBool {
        let (Some(o), Some(k), Some(n)) = (nid(object), cstr_bytes(string), nid(newitem)) else { return 0 };
        let mut st = store();
        if dom::replace_item_in_object(&mut st, o, k, n, true) { 1 } else { 0 }
    }
}

export! {
    pub unsafe fn cJSON_Duplicate(item: *const CJson, recurse: CJsonBool) -> *mut CJson {
        let Some(i) = nid(item) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::duplicate(&mut st, i, recurse != 0))
    }
}

export! {
    pub unsafe fn cJSON_Compare(a: *const CJson, b: *const CJson, case_sensitive: CJsonBool) -> CJsonBool {
        let (Some(x), Some(y)) = (nid(a), nid(b)) else { return 0 };
        let st = store();
        if cjson_core::compare::compare(&st, x, y, case_sensitive != 0) { 1 } else { 0 }
    }
}

export! {
    pub unsafe fn cJSON_Minify(json: *mut c_char) {
        if json.is_null() {
            return;
        }
        let len = libc_strlen(json);
        let buf = std::slice::from_raw_parts_mut(json as *mut u8, len + 1);
        let _ = cjson_core::minify(buf);
    }
}

export! {
    pub unsafe fn cJSON_AddNullToObject(object: *mut CJson, name: *const c_char) -> *mut CJson {
        let (Some(o), Some(n)) = (nid(object), cstr_bytes(name)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::add_null_to_object(&mut st, o, n))
    }
}
export! {
    pub unsafe fn cJSON_AddTrueToObject(object: *mut CJson, name: *const c_char) -> *mut CJson {
        let (Some(o), Some(n)) = (nid(object), cstr_bytes(name)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::add_true_to_object(&mut st, o, n))
    }
}
export! {
    pub unsafe fn cJSON_AddFalseToObject(object: *mut CJson, name: *const c_char) -> *mut CJson {
        let (Some(o), Some(n)) = (nid(object), cstr_bytes(name)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::add_false_to_object(&mut st, o, n))
    }
}
export! {
    pub unsafe fn cJSON_AddBoolToObject(object: *mut CJson, name: *const c_char, boolean: CJsonBool) -> *mut CJson {
        let (Some(o), Some(n)) = (nid(object), cstr_bytes(name)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::add_bool_to_object(&mut st, o, n, boolean != 0))
    }
}
export! {
    pub unsafe fn cJSON_AddNumberToObject(object: *mut CJson, name: *const c_char, number: c_double) -> *mut CJson {
        let (Some(o), Some(n)) = (nid(object), cstr_bytes(name)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::add_number_to_object(&mut st, o, n, number))
    }
}
export! {
    pub unsafe fn cJSON_AddStringToObject(object: *mut CJson, name: *const c_char, string: *const c_char) -> *mut CJson {
        let (Some(o), Some(n), Some(s)) = (nid(object), cstr_bytes(name), cstr_bytes(string)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::add_string_to_object(&mut st, o, n, s))
    }
}
export! {
    pub unsafe fn cJSON_AddRawToObject(object: *mut CJson, name: *const c_char, raw: *const c_char) -> *mut CJson {
        let (Some(o), Some(n), Some(s)) = (nid(object), cstr_bytes(name), cstr_bytes(raw)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::add_raw_to_object(&mut st, o, n, s))
    }
}
export! {
    pub unsafe fn cJSON_AddObjectToObject(object: *mut CJson, name: *const c_char) -> *mut CJson {
        let (Some(o), Some(n)) = (nid(object), cstr_bytes(name)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::add_object_to_object(&mut st, o, n))
    }
}
export! {
    pub unsafe fn cJSON_AddArrayToObject(object: *mut CJson, name: *const c_char) -> *mut CJson {
        let (Some(o), Some(n)) = (nid(object), cstr_bytes(name)) else { return std::ptr::null_mut() };
        let mut st = store();
        out(dom::add_array_to_object(&mut st, o, n))
    }
}

export! {
    pub unsafe fn cJSON_SetNumberHelper(object: *mut CJson, number: c_double) -> c_double {
        let Some(o) = nid(object) else { return f64::NAN };
        let mut st = store();
        dom::set_number_helper(&mut st, o, number)
    }
}

export! {
    pub unsafe fn cJSON_SetValuestring(object: *mut CJson, valuestring: *const c_char) -> *mut c_char {
        let (Some(o), Some(s)) = (nid(object), cstr_bytes(valuestring)) else {
            return std::ptr::null_mut();
        };
        let mut st = store();
        if !dom::set_valuestring(&mut st, o, s) {
            return std::ptr::null_mut();
        }
        (*object).valuestring
    }
}

export! {
    pub unsafe fn cJSON_malloc(size: usize) -> *mut c_void {
        hooks::malloc_user(size)
    }
}

export! {
    pub unsafe fn cJSON_free(object: *mut c_void) {
        hooks::free_user(object);
    }
}
