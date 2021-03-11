use crate::collections::{BTreeMap, LinkedList};
use crate::ptr;
use crate::sys::ffi::*;
use crate::sys_common::rwlock::RWLock;

pub type Key = usize;
pub type Tls = BTreeMap<Key, *mut u8>;

const TLS_INDEX: BaseType_t = 1;

pub static LOCK: RWLock = RWLock::new();
pub static mut KEYS: LinkedList<(Key, Option<unsafe extern "C" fn(*mut u8)>)> = LinkedList::new();

#[inline]
pub unsafe fn get_tls() -> *mut Tls {
    pvTaskGetThreadLocalStoragePointer(ptr::null_mut(), TLS_INDEX) as *mut Tls
}

#[inline]
pub unsafe fn set_tls(tls: *mut Tls) {
    vTaskSetThreadLocalStoragePointer(ptr::null_mut(), TLS_INDEX, tls as *mut _);
}
