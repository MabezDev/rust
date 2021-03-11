use crate::ffi::CStr;
use crate::io;
use crate::mem;
use crate::ptr;
use crate::sync::{
    atomic::{AtomicU8, Ordering::SeqCst},
    Arc,
};
use crate::sys::mutex::Mutex;
use crate::sys::{ffi::*, stack_overflow, thread_local_dtor};
use crate::time::Duration;

const PENDING: u8 = 0;
const RUNNING: u8 = 1;
const DETACHED: u8 = 2;
const EXITED: u8 = 3;

pub const DEFAULT_MIN_STACK_SIZE: usize = 4096;

pub struct Thread {
    id: TaskHandle_t,
    join_mutex: Arc<Mutex>,
    state: Arc<AtomicU8>,
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

impl Thread {
    // unsafe: see thread::Builder::spawn_unchecked for safety requirements
    pub unsafe fn new(
        name: Option<&CStr>,
        stack: usize,
        p: Box<dyn FnOnce()>,
    ) -> io::Result<Thread> {
        let join_mutex = Arc::new(Mutex::new());
        let state = Arc::new(AtomicU8::new(PENDING));

        let arg = box (Arc::clone(&join_mutex), Arc::clone(&state), p);

        let name = name.unwrap_or_else(|| CStr::from_bytes_with_nul_unchecked(b"\0"));

        let mut thread = Thread { id: ptr::null_mut(), join_mutex, state };

        let arg = Box::into_raw(arg);

        let res = xTaskCreate(
            thread_start,
            name.as_ptr(),
            stack as u32,
            arg as *mut libc::c_void,
            5,
            &mut thread.id,
        );

        if res != pdTRUE {
            drop(Box::from_raw(arg));

            if res == errCOULD_NOT_ALLOCATE_REQUIRED_MEMORY {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "could not allocate required memory for thread",
                ));
            } else {
                return Err(io::ErrorKind::WouldBlock.into());
            }
        }

        return Ok(thread);

        extern "C" fn thread_start(arg: *mut libc::c_void) -> *mut libc::c_void {
            unsafe {
                let previous_state = {
                    let _handler = stack_overflow::Handler::new();

                    let (join_mutex, state, main) =
                        *Box::from_raw(arg as *mut (Arc<Mutex>, Arc<AtomicU8>, Box<dyn FnOnce()>));

                    join_mutex.lock();
                    let _ = state.compare_exchange(PENDING, RUNNING, SeqCst, SeqCst);

                    main();
                    thread_local_dtor::run_dtors();

                    let previous_state = match state.compare_exchange(RUNNING, EXITED, SeqCst, SeqCst) {
                      Ok(v) => v,
                      Err(v) => v,
                    };

                    join_mutex.unlock();

                    previous_state
                };

                if previous_state == DETACHED {
                    drop(previous_state);
                    vTaskDelete(ptr::null_mut());
                } else {
                    drop(previous_state);
                    vTaskSuspend(ptr::null_mut());
                }
            }

            ptr::null_mut()
        }
    }

    pub fn yield_now() {
        unsafe { vTaskDelay(0) };
    }

    pub fn sleep(dur: Duration) {
        use crate::cmp;

        let nanos = dur.as_nanos();

        if nanos == 0 {
            return;
        }

        let millis;

        #[cfg(target_arch = "xtensa")]
        {
            extern "C" {
                fn ets_delay_us(us: u32);
            }

            let mut micros = cmp::max(1_000, nanos) / 1_000;
            let amt = micros % 1_000;

            if amt > 0 {
                micros -= amt;
                unsafe { ets_delay_us(amt as u32) };
            }

            if micros == 0 {
                return;
            }

            millis = cmp::max(1_000, micros) / 1_000;
        }

        #[cfg(not(target_arch = "xtensa"))]
        {
            millis = cmp::max(1_000_000, nanos) / 1_000_000;
        }

        let tick_rate = unsafe { xPortGetTickRateHz() } as u128;
        let ms_per_tick = 1000 / tick_rate;
        let mut ticks_to_delay = (millis + ms_per_tick - 1) / ms_per_tick;

        while ticks_to_delay > 0 {
            let amt = cmp::min(u32::max_value() as u128, ticks_to_delay);
            ticks_to_delay -= amt;
            unsafe { vTaskDelay(amt as u32) };
        }
    }

    pub fn join(self) {
        unsafe {
            assert!(self.id != xTaskGetCurrentTaskHandle());

            while self.state.load(SeqCst) == PENDING {
                Self::yield_now()
            }

            // Just wait for the thread to finish, the rest is handled by `Drop`.
            self.join_mutex.lock();
            self.join_mutex.unlock();
        }
    }

    pub fn id(&self) -> TaskHandle_t {
        self.id
    }

    pub fn into_id(self) -> TaskHandle_t {
        let id = self.id;
        mem::forget(self);
        id
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        // If the thread already exited before we could set the state to `DETACHED`, delete it manually here.
        if self.state.swap(DETACHED, SeqCst) == EXITED {
            unsafe {
                vTaskDelete(self.id);
            }
        }
    }
}

#[cfg_attr(test, allow(dead_code))]
pub mod guard {
    use crate::ops::Range;
    pub type Guard = Range<usize>;
    pub unsafe fn current() -> Option<Guard> {
        None
    }
    pub unsafe fn init() -> Option<Guard> {
        None
    }
}
