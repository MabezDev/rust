use crate::ptr;

mod thread_local;
use thread_local::{get_tls, set_tls, Tls, KEYS, LOCK};

pub use thread_local::Key;

#[inline]
pub unsafe fn create(dtor: Option<unsafe extern "C" fn(*mut u8)>) -> Key {
    LOCK.write();

    let key = KEYS.back().map(|&(i, _)| i + 1).unwrap_or(1);
    KEYS.push_back((key, dtor));

    LOCK.write_unlock();

    key
}

#[inline]
pub unsafe fn set(key: Key, value: *mut u8) {
    #[cfg(debug_assertions)]
    {
        LOCK.read();

        assert!(KEYS.iter().any(|&(k, _)| k == key));

        LOCK.read_unlock();
    }

    let tls = if let Some(tls) = get_tls().as_mut() {
        tls
    } else {
        let tls = Box::into_raw(box Tls::new());
        set_tls(tls);
        &mut *tls
    };

    if value.is_null() {
        tls.remove(&key);
    } else {
        tls.insert(key, value);
    }
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    if let Some(tls) = get_tls().as_mut() {
        tls.get(&key).cloned().unwrap_or_else(ptr::null_mut)
    } else {
        ptr::null_mut()
    }
}

#[inline]
pub unsafe fn destroy(key: Key) {
    LOCK.write();

    if let Some(i) = KEYS.iter().position(|&(k, _)| k == key) {
        KEYS.remove(i);
    } else {
      debug_assert!(false);
      core::hint::unreachable_unchecked();
    }

    LOCK.write_unlock();
}

#[inline]
pub fn requires_synchronized_create() -> bool {
    false
}
