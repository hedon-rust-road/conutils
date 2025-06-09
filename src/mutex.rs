use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, Ordering},
};

use atomic_wait::{wait, wake_one};

pub struct Mutex<T> {
    /// 0: unlocked
    /// 1: locked, no other threads waiting
    /// 2: unlocked, other threads waiting
    state: AtomicU32,
    value: UnsafeCell<T>,
}

pub struct MutexGuard<'a, T> {
    pub(crate) mutex: &'a Mutex<T>,
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        lock_contended(&self.state);
        // Swap successfully, means locked.
        MutexGuard { mutex: self }
    }
}

fn lock_contended(state: &AtomicU32) {
    let mut spin_count = 0;
    while let Err(s) = state.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
        if s == 1 {
            if spin_count < 100 {
                spin_count += 1;
                std::hint::spin_loop();
                continue;
            }
            _ = state.compare_exchange(1, 2, Ordering::Acquire, Ordering::Relaxed);
        }
        wait(state, 2)
    }
}

unsafe impl<T> Sync for Mutex<T> where T: Send {}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.value.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.value.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // If there are threads waiting for the lock, wait one of them.
        if self.mutex.state.swap(0, Ordering::Release) == 2 {
            wake_one(&self.mutex.state);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        thread::{self, sleep},
        time::Duration,
    };

    use super::*;

    #[test]
    fn one_thread_should_work() {
        let l = Mutex::new(vec![]);
        let mut guard = l.lock();
        guard.push(1);
        drop(guard);

        let guard = l.lock();
        assert_eq!(guard[0], 1);
    }

    #[test]
    fn cross_thread_should_work() {
        let l = Mutex::new(vec![]);

        thread::scope(|s| {
            s.spawn(|| {
                let mut guard = l.lock();
                guard.push(1);
                sleep(Duration::from_millis(100)); // sleep for making the second thread to be blcoked.
            });

            sleep(Duration::from_millis(10)); // make sure the first thread get the lock
            s.spawn(|| {
                let mut guard = l.lock();
                guard.push(2);
            });
        });

        let guard = l.lock();
        assert_eq!(guard.len(), 2);
    }

    #[test]
    fn high_concurrency_test() {
        let l = Mutex::new(0);

        std::thread::scope(|s| {
            for _ in 0..10 {
                s.spawn(|| {
                    for _ in 0..1000 {
                        let mut guard = l.lock();
                        *guard += 1;
                    }
                });
            }
        });

        let guard = l.lock();
        assert_eq!(*guard, 10 * 1000);
    }
}
