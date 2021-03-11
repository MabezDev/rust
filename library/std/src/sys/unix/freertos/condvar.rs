use crate::cell::UnsafeCell;
use crate::collections::VecDeque;
use crate::sys::mutex::{Mutex, ReentrantMutex};
use crate::time::Duration;

use crate::sys::ffi::*;

pub struct Condvar {
    lock: ReentrantMutex,
    waiter_list: UnsafeCell<Option<VecDeque<SemaphoreHandle_t>>>,
}

pub type MovableCondvar = Condvar;

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    pub const fn new() -> Condvar {
        Condvar {
            lock: unsafe { ReentrantMutex::uninitialized() },
            waiter_list: UnsafeCell::new(None),
        }
    }

    #[inline]
    pub unsafe fn init(&mut self) {
        // This must stay empty since `RWLock` uses a `Condvar`
        // internally which cannot be mutated.
    }

    #[inline]
    pub unsafe fn notify_one(&self) {
        self.lock.lock();

        if let Some(waiter_list) = (&*self.waiter_list.get()).as_ref() {
            if let Some(&waiter) = waiter_list.front() {
                xSemaphoreGive(waiter);
            }
        }

        self.lock.unlock();
    }

    #[inline]
    pub unsafe fn notify_all(&self) {
        self.lock.lock();

        if let Some(waiter_list) = (&*self.waiter_list.get()).as_ref() {
            for &waiter in waiter_list {
                xSemaphoreGive(waiter);
            }
        }

        self.lock.unlock();
    }

    #[inline]
    pub unsafe fn wait(&self, mutex: &Mutex) {
        let r = self.wait_timeout_option(mutex, None);
        debug_assert_eq!(r, true);
    }

    #[inline]
    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        self.wait_timeout_option(mutex, Some(dur))
    }

    unsafe fn wait_timeout_option(&self, mutex: &Mutex, dur: Option<Duration>) -> bool {
        use crate::time::Instant;

        let (now, timeout_ticks) = if let Some(dur) = dur {
            let now = Instant::now();
            let timeout_ms = dur.as_millis() as TickType_t;
            let portTICK_PERIOD_MS = 1000 / xPortGetTickRateHz();
            (Some(now), timeout_ms / portTICK_PERIOD_MS)
        } else {
            (None, TickType_t::max_value())
        };

        let waiter = xSemaphoreCreateCounting(1, 0);

        self.lock.lock();

        let waiter_list = (&mut *self.waiter_list.get()).get_or_insert_with(|| VecDeque::new());
        waiter_list.push_back(waiter);

        self.lock.unlock();

        mutex.unlock();

        let r = if xSemaphoreTake(waiter, timeout_ticks) == pdTRUE {
            true
        } else {
            false
        };

        self.lock.lock();

        if let Some(index) = waiter_list.iter().position(|&w| w == waiter) {
            waiter_list.remove(index);
            vSemaphoreDelete(waiter);
        } else {
            core::hint::unreachable_unchecked();
        }

        self.lock.unlock();

        mutex.lock();

        if let (Some(now), Some(dur)) = (now, dur) {
            r && now.elapsed() < dur
        } else {
            r
        }
    }

    #[inline]
    pub unsafe fn destroy(&self) {
        #[cfg(debug_assertions)]
        {
            self.lock.lock();

            if let Some(waiter_list) = (&*self.waiter_list.get()).as_ref() {
                assert!(waiter_list.is_empty());
            }

            self.lock.unlock();
        }
    }
}
