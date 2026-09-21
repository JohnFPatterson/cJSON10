// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

use crate::number::compare_double;
use crate::store::{NodeId, Store};
use crate::types::{self, type_kind};

fn get_object_item<S: Store>(
    store: &S,
    object: NodeId,
    name: Option<&[u8]>,
    case_sensitive: bool,
) -> Option<NodeId> {
    let Some(name) = name else {
        return None;
    };
    let mut cur = store.child(object);
    while let Some(el) = cur {
        if crate::types::bytes_eq_case(store.key_bytes(el), Some(name), case_sensitive)
            && store.key_bytes(el).is_some()
        {
            return Some(el);
        }
        cur = store.next(el);
    }
    None
}

pub fn compare<S: Store>(store: &S, a: NodeId, b: NodeId, case_sensitive: bool) -> bool {
    if type_kind(store.ty(a)) != type_kind(store.ty(b)) {
        return false;
    }
    match type_kind(store.ty(a)) {
        types::FALSE | types::TRUE | types::NULL | types::NUMBER | types::STRING | types::RAW
        | types::ARRAY | types::OBJECT => {}
        _ => return false,
    }
    if a == b {
        return true;
    }
    match type_kind(store.ty(a)) {
        types::FALSE | types::TRUE | types::NULL => true,
        types::NUMBER => compare_double(store.valuedouble(a), store.valuedouble(b)),
        types::STRING | types::RAW => match (store.valuestring_bytes(a), store.valuestring_bytes(b))
        {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
        types::ARRAY => {
            let mut ae = store.child(a);
            let mut be = store.child(b);
            while let (Some(x), Some(y)) = (ae, be) {
                if !compare(store, x, y, case_sensitive) {
                    return false;
                }
                ae = store.next(x);
                be = store.next(y);
            }
            ae.is_none() && be.is_none()
        }
        types::OBJECT => {
            let mut ae = store.child(a);
            while let Some(x) = ae {
                let key = store.key_bytes(x).map(|k| k.to_vec());
                let Some(y) = get_object_item(store, b, key.as_deref(), case_sensitive) else {
                    return false;
                };
                if !compare(store, x, y, case_sensitive) {
                    return false;
                }
                ae = store.next(x);
            }
            let mut be = store.child(b);
            while let Some(y) = be {
                let key = store.key_bytes(y).map(|k| k.to_vec());
                let Some(x) = get_object_item(store, a, key.as_deref(), case_sensitive) else {
                    return false;
                };
                if !compare(store, y, x, case_sensitive) {
                    return false;
                }
                be = store.next(y);
            }
            true
        }
        _ => false,
    }
}

/// Utils `compare_json`: numbers compare valueint AND valuedouble; objects are sorted first.
pub fn compare_json<S: Store>(
    store: &mut S,
    a: Option<NodeId>,
    b: Option<NodeId>,
    case_sensitive: bool,
) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    if type_kind(store.ty(a)) != type_kind(store.ty(b)) {
        return false;
    }
    match type_kind(store.ty(a)) {
        types::NUMBER => {
            store.valueint(a) == store.valueint(b)
                && compare_double(store.valuedouble(a), store.valuedouble(b))
        }
        types::STRING => match (store.valuestring_bytes(a), store.valuestring_bytes(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
        types::ARRAY => {
            let mut ae = store.child(a);
            let mut be = store.child(b);
            while let (Some(x), Some(y)) = (ae, be) {
                if !compare_json(store, Some(x), Some(y), case_sensitive) {
                    return false;
                }
                ae = store.next(x);
                be = store.next(y);
            }
            ae.is_none() && be.is_none()
        }
        types::OBJECT => {
            crate::utils::sort_object(store, a, case_sensitive);
            crate::utils::sort_object(store, b, case_sensitive);
            let mut ae = store.child(a);
            let mut be = store.child(b);
            while let (Some(x), Some(y)) = (ae, be) {
                if !crate::types::bytes_eq_case(
                    store.key_bytes(x),
                    store.key_bytes(y),
                    case_sensitive,
                ) {
                    return false;
                }
                if !compare_json(store, Some(x), Some(y), case_sensitive) {
                    return false;
                }
                ae = store.next(x);
                be = store.next(y);
            }
            ae.is_none() && be.is_none()
        }
        _ => true,
    }
}
