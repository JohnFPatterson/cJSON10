// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;

use cjson_core::store::{NodeId, Store, StrId};

use crate::ffi::CJson;
use crate::hooks;

pub struct CStore;

impl CStore {
    pub fn node_from_ptr(p: *mut CJson) -> Option<NodeId> {
        NodeId::from_raw(p as usize)
    }
    pub fn ptr_from_node(id: NodeId) -> *mut CJson {
        id.raw() as *mut CJson
    }
    pub fn str_from_ptr(p: *const c_char) -> Option<StrId> {
        StrId::from_raw(p as usize)
    }
    pub fn ptr_from_str(id: StrId) -> *mut c_char {
        id.raw() as *mut c_char
    }

    unsafe fn node<'a>(&self, id: NodeId) -> &'a CJson {
        &*Self::ptr_from_node(id)
    }
    unsafe fn node_mut<'a>(&mut self, id: NodeId) -> &'a mut CJson {
        &mut *Self::ptr_from_node(id)
    }
}

impl Store for CStore {
    fn alloc_node(&mut self) -> Option<NodeId> {
        unsafe {
            let p = hooks::allocate(std::mem::size_of::<CJson>()) as *mut CJson;
            if p.is_null() {
                return None;
            }
            ptr::write_bytes(p, 0, 1);
            Self::node_from_ptr(p)
        }
    }

    fn free_node_only(&mut self, id: NodeId) {
        unsafe {
            hooks::deallocate(Self::ptr_from_node(id) as *mut _);
        }
    }

    fn ty(&self, id: NodeId) -> i32 {
        unsafe { self.node(id).type_ }
    }
    fn set_ty(&mut self, id: NodeId, ty: i32) {
        unsafe { self.node_mut(id).type_ = ty }
    }
    fn valueint(&self, id: NodeId) -> i32 {
        unsafe { self.node(id).valueint }
    }
    fn set_valueint(&mut self, id: NodeId, v: i32) {
        unsafe { self.node_mut(id).valueint = v }
    }
    fn valuedouble(&self, id: NodeId) -> f64 {
        unsafe { self.node(id).valuedouble }
    }
    fn set_valuedouble(&mut self, id: NodeId, v: f64) {
        unsafe { self.node_mut(id).valuedouble = v }
    }

    fn next(&self, id: NodeId) -> Option<NodeId> {
        unsafe { Self::node_from_ptr(self.node(id).next) }
    }
    fn set_next(&mut self, id: NodeId, n: Option<NodeId>) {
        unsafe {
            self.node_mut(id).next = n.map(Self::ptr_from_node).unwrap_or(ptr::null_mut());
        }
    }
    fn prev(&self, id: NodeId) -> Option<NodeId> {
        unsafe { Self::node_from_ptr(self.node(id).prev) }
    }
    fn set_prev(&mut self, id: NodeId, n: Option<NodeId>) {
        unsafe {
            self.node_mut(id).prev = n.map(Self::ptr_from_node).unwrap_or(ptr::null_mut());
        }
    }
    fn child(&self, id: NodeId) -> Option<NodeId> {
        unsafe { Self::node_from_ptr(self.node(id).child) }
    }
    fn set_child(&mut self, id: NodeId, n: Option<NodeId>) {
        unsafe {
            self.node_mut(id).child = n.map(Self::ptr_from_node).unwrap_or(ptr::null_mut());
        }
    }

    fn key_bytes(&self, id: NodeId) -> Option<&[u8]> {
        unsafe {
            let p = self.node(id).string;
            if p.is_null() {
                return None;
            }
            Some(CStr::from_ptr(p).to_bytes())
        }
    }
    fn valuestring_bytes(&self, id: NodeId) -> Option<&[u8]> {
        unsafe {
            let p = self.node(id).valuestring;
            if p.is_null() {
                return None;
            }
            Some(CStr::from_ptr(p).to_bytes())
        }
    }
    fn key_id(&self, id: NodeId) -> Option<StrId> {
        unsafe { Self::str_from_ptr(self.node(id).string) }
    }
    fn valuestring_id(&self, id: NodeId) -> Option<StrId> {
        unsafe { Self::str_from_ptr(self.node(id).valuestring) }
    }
    fn set_key_id(&mut self, id: NodeId, s: Option<StrId>) {
        unsafe {
            self.node_mut(id).string = s.map(Self::ptr_from_str).unwrap_or(ptr::null_mut());
        }
    }
    fn set_valuestring_id(&mut self, id: NodeId, s: Option<StrId>) {
        unsafe {
            self.node_mut(id).valuestring = s.map(Self::ptr_from_str).unwrap_or(ptr::null_mut());
        }
    }

    fn alloc_str(&mut self, bytes: &[u8]) -> Option<StrId> {
        unsafe {
            let p = hooks::allocate(bytes.len() + 1) as *mut u8;
            if p.is_null() {
                return None;
            }
            ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            *p.add(bytes.len()) = 0;
            StrId::from_raw(p as usize)
        }
    }

    fn borrow_str(&mut self, bytes: &[u8]) -> Option<StrId> {
        StrId::from_raw(bytes.as_ptr() as usize)
    }

    fn free_str(&mut self, id: StrId) {
        unsafe {
            hooks::deallocate(Self::ptr_from_str(id) as *mut _);
        }
    }

    fn copy_fields(&mut self, dest: NodeId, src: NodeId) {
        unsafe {
            ptr::copy_nonoverlapping(Self::ptr_from_node(src), Self::ptr_from_node(dest), 1);
        }
    }
}

pub fn cstr_bytes<'a>(p: *const c_char) -> Option<&'a [u8]> {
    if p.is_null() {
        return None;
    }
    unsafe { Some(CStr::from_ptr(p).to_bytes()) }
}

pub fn slice_from_ptr_len<'a>(p: *const c_char, len: usize) -> Option<&'a [u8]> {
    if p.is_null() || len == 0 {
        return None;
    }
    unsafe { Some(std::slice::from_raw_parts(p as *const u8, len)) }
}
