use crate::ptr;

mod thread_local;
use thread_local::{LOCK, KEYS, get_tls, set_tls};

#[inline]
pub unsafe fn run_dtors() {
    if let Some(tls) = get_tls().as_mut() {
        let tls = Box::from_raw(tls);

        LOCK.read();

        for (k, dtor) in KEYS.iter() {
            if let (Some(v), Some(dtor)) = (tls.get(k), dtor) {
                dtor(*v)
            }
        }

        LOCK.read_unlock();

        set_tls(ptr::null_mut());
    }
}

