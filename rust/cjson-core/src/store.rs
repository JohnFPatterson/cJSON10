// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! Opaque graph store. The C ABI shim implements this with malloc'd `cJSON` nodes.

use crate::types::{is_reference, string_is_const};

/// Handle of a DOM node. Zero is reserved for NULL; the shim typically stores a C pointer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(pub core::num::NonZeroUsize);

/// Handle of a NUL-terminated byte string allocated by the store.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StrId(pub core::num::NonZeroUsize);

impl NodeId {
    #[inline]
    pub fn from_raw(raw: usize) -> Option<Self> {
        core::num::NonZeroUsize::new(raw).map(Self)
    }

    #[inline]
    pub fn raw(self) -> usize {
        self.0.get()
    }
}

impl StrId {
    #[inline]
    pub fn from_raw(raw: usize) -> Option<Self> {
        core::num::NonZeroUsize::new(raw).map(Self)
    }

    #[inline]
    pub fn raw(self) -> usize {
        self.0.get()
    }
}

/// Graph used by every core algorithm. Implementations must not require `unsafe` in this crate.
pub trait Store {
    fn alloc_node(&mut self) -> Option<NodeId>;
    /// Free the node struct only (not children or strings).
    fn free_node_only(&mut self, id: NodeId);

    fn ty(&self, id: NodeId) -> i32;
    fn set_ty(&mut self, id: NodeId, ty: i32);
    fn valueint(&self, id: NodeId) -> i32;
    fn set_valueint(&mut self, id: NodeId, v: i32);
    fn valuedouble(&self, id: NodeId) -> f64;
    fn set_valuedouble(&mut self, id: NodeId, v: f64);

    fn next(&self, id: NodeId) -> Option<NodeId>;
    fn set_next(&mut self, id: NodeId, n: Option<NodeId>);
    fn prev(&self, id: NodeId) -> Option<NodeId>;
    fn set_prev(&mut self, id: NodeId, n: Option<NodeId>);
    fn child(&self, id: NodeId) -> Option<NodeId>;
    fn set_child(&mut self, id: NodeId, n: Option<NodeId>);

    fn key_bytes(&self, id: NodeId) -> Option<&[u8]>;
    fn valuestring_bytes(&self, id: NodeId) -> Option<&[u8]>;
    fn key_id(&self, id: NodeId) -> Option<StrId>;
    fn valuestring_id(&self, id: NodeId) -> Option<StrId>;
    fn set_key_id(&mut self, id: NodeId, s: Option<StrId>);
    fn set_valuestring_id(&mut self, id: NodeId, s: Option<StrId>);

    /// Allocate a NUL-terminated copy of `bytes` (no interior NUL required).
    fn alloc_str(&mut self, bytes: &[u8]) -> Option<StrId>;
    /// Adopt an existing C string without copying (reference / const key).
    fn borrow_str(&mut self, bytes: &[u8]) -> Option<StrId>;
    fn free_str(&mut self, id: StrId);

    /// Copy every field from `src` onto `dest` (like `memcpy` of the C struct).
    fn copy_fields(&mut self, dest: NodeId, src: NodeId);

    /// cJSON_Delete: walk `next`, honoring IsReference / StringIsConst.
    fn delete(&mut self, mut item: Option<NodeId>) {
        while let Some(id) = item {
            let next = self.next(id);
            let ty = self.ty(id);
            if !is_reference(ty) {
                if let Some(child) = self.child(id) {
                    self.delete(Some(child));
                }
                if let Some(s) = self.valuestring_id(id) {
                    self.set_valuestring_id(id, None);
                    self.free_str(s);
                }
            }
            if !string_is_const(ty) {
                if let Some(s) = self.key_id(id) {
                    self.set_key_id(id, None);
                    self.free_str(s);
                }
            }
            self.free_node_only(id);
            item = next;
        }
    }

    /// overwrite_item + free wrapper: transfer `src` into `dest`, then free `src`'s node only.
    fn steal_into(&mut self, dest: NodeId, src: NodeId) {
        if let Some(s) = self.key_id(dest) {
            self.set_key_id(dest, None);
            self.free_str(s);
        }
        if let Some(s) = self.valuestring_id(dest) {
            self.set_valuestring_id(dest, None);
            self.free_str(s);
        }
        if let Some(c) = self.child(dest) {
            self.delete(Some(c));
            self.set_child(dest, None);
        }
        self.copy_fields(dest, src);
        self.free_node_only(src);
    }
}

#[derive(Clone, Default)]
struct Node {
    next: Option<NodeId>,
    prev: Option<NodeId>,
    child: Option<NodeId>,
    ty: i32,
    valuestring: Option<StrId>,
    valueint: i32,
    valuedouble: f64,
    key: Option<StrId>,
}

/// In-memory store for core unit tests. No raw pointers.
pub struct SafeStore {
    nodes: Vec<Option<Node>>,
    strings: Vec<Option<Vec<u8>>>,
    pub fail_allocs: bool,
}

impl Default for SafeStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SafeStore {
    pub fn new() -> Self {
        Self {
            nodes: vec![None],
            strings: vec![None],
            fail_allocs: false,
        }
    }

    fn node(&self, id: NodeId) -> &Node {
        self.nodes[id.raw()].as_ref().expect("dangling NodeId")
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes[id.raw()].as_mut().expect("dangling NodeId")
    }
}

impl Store for SafeStore {
    fn alloc_node(&mut self) -> Option<NodeId> {
        if self.fail_allocs {
            return None;
        }
        for (i, slot) in self.nodes.iter().enumerate().skip(1) {
            if slot.is_none() {
                self.nodes[i] = Some(Node::default());
                return NodeId::from_raw(i);
            }
        }
        self.nodes.push(Some(Node::default()));
        NodeId::from_raw(self.nodes.len() - 1)
    }

    fn free_node_only(&mut self, id: NodeId) {
        self.nodes[id.raw()] = None;
    }

    fn ty(&self, id: NodeId) -> i32 {
        self.node(id).ty
    }
    fn set_ty(&mut self, id: NodeId, ty: i32) {
        self.node_mut(id).ty = ty;
    }
    fn valueint(&self, id: NodeId) -> i32 {
        self.node(id).valueint
    }
    fn set_valueint(&mut self, id: NodeId, v: i32) {
        self.node_mut(id).valueint = v;
    }
    fn valuedouble(&self, id: NodeId) -> f64 {
        self.node(id).valuedouble
    }
    fn set_valuedouble(&mut self, id: NodeId, v: f64) {
        self.node_mut(id).valuedouble = v;
    }

    fn next(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).next
    }
    fn set_next(&mut self, id: NodeId, n: Option<NodeId>) {
        self.node_mut(id).next = n;
    }
    fn prev(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).prev
    }
    fn set_prev(&mut self, id: NodeId, n: Option<NodeId>) {
        self.node_mut(id).prev = n;
    }
    fn child(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).child
    }
    fn set_child(&mut self, id: NodeId, n: Option<NodeId>) {
        self.node_mut(id).child = n;
    }

    fn key_bytes(&self, id: NodeId) -> Option<&[u8]> {
        let sid = self.node(id).key?;
        self.strings[sid.raw()].as_deref()
    }
    fn valuestring_bytes(&self, id: NodeId) -> Option<&[u8]> {
        let sid = self.node(id).valuestring?;
        self.strings[sid.raw()].as_deref()
    }
    fn key_id(&self, id: NodeId) -> Option<StrId> {
        self.node(id).key
    }
    fn valuestring_id(&self, id: NodeId) -> Option<StrId> {
        self.node(id).valuestring
    }
    fn set_key_id(&mut self, id: NodeId, s: Option<StrId>) {
        self.node_mut(id).key = s;
    }
    fn set_valuestring_id(&mut self, id: NodeId, s: Option<StrId>) {
        self.node_mut(id).valuestring = s;
    }

    fn alloc_str(&mut self, bytes: &[u8]) -> Option<StrId> {
        if self.fail_allocs {
            return None;
        }
        for (i, slot) in self.strings.iter().enumerate().skip(1) {
            if slot.is_none() {
                self.strings[i] = Some(bytes.to_vec());
                return StrId::from_raw(i);
            }
        }
        self.strings.push(Some(bytes.to_vec()));
        StrId::from_raw(self.strings.len() - 1)
    }

    fn borrow_str(&mut self, bytes: &[u8]) -> Option<StrId> {
        // SafeStore cannot hold an external pointer; copy and treat as a normal string.
        // Callers set STRING_IS_CONST / IS_REFERENCE so Delete will not free it if required.
        self.alloc_str(bytes)
    }

    fn free_str(&mut self, id: StrId) {
        if id.raw() < self.strings.len() {
            self.strings[id.raw()] = None;
        }
    }

    fn copy_fields(&mut self, dest: NodeId, src: NodeId) {
        let cloned = self.node(src).clone();
        *self.node_mut(dest) = cloned;
    }
}

pub fn key_owned<S: Store>(store: &S, id: NodeId) -> Option<Vec<u8>> {
    store.key_bytes(id).map(|b| b.to_vec())
}

pub fn valuestring_owned<S: Store>(store: &S, id: NodeId) -> Option<Vec<u8>> {
    store.valuestring_bytes(id).map(|b| b.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types;

    #[test]
    fn alloc_and_delete_tree() {
        let mut s = SafeStore::new();
        let root = s.alloc_node().unwrap();
        s.set_ty(root, types::ARRAY);
        let child = s.alloc_node().unwrap();
        s.set_ty(child, types::NULL);
        s.set_child(root, Some(child));
        s.delete(Some(root));
        assert!(s.alloc_node().is_some());
    }

    #[test]
    fn fail_allocs() {
        let mut s = SafeStore::new();
        s.fail_allocs = true;
        assert!(s.alloc_node().is_none());
    }
}
