use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, Ordering},
};

use atomic_wait::{wait, wake_all, wake_one};

pub struct RwMutex<T> {
    /// The number of readers, or u32::MAX if write-locked.
    state: AtomicU32,
    /// Incremented to wake up writers.
    write_wake_counter: AtomicU32,
    value: UnsafeCell<T>,
}

pub struct ReadGuard<'a, T> {
    rwmutex: &'a RwMutex<T>,
}

pub struct WriteGuard<'a, T> {
    rwmutx: &'a RwMutex<T>,
}

unsafe impl<T> Sync for RwMutex<T> where T: Send + Sync {}

impl<T> RwMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            write_wake_counter: AtomicU32::new(0),
            value: UnsafeCell::new(value),
        }
    }

    pub fn read(&self) -> ReadGuard<T> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            // unlocked or read locked
            if s < u32::MAX {
                assert!(s != u32::MAX - 1, "too many readers");
                match self
                    .state
                    .compare_exchange(s, s + 1, Ordering::Acquire, Ordering::Relaxed)
                {
                    Ok(_) => return ReadGuard { rwmutex: self },
                    Err(e) => s = e,
                }
            }
            // write locked
            if s == u32::MAX {
                wait(&self.state, u32::MAX);
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }

    pub fn write(&self) -> WriteGuard<T> {
        while let Err(s) =
            self.state
                .compare_exchange(0, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
        {
            let w = self.write_wake_counter.load(Ordering::Acquire);
            if self.state.load(Ordering::Relaxed) != 0 {
                // Wait if the RwMutex is still locked, but only i
                // there have been no wake signals since we checked.
                wait(&self.write_wake_counter, w);
            }
            wait(&self.state, s);
        }
        WriteGuard { rwmutx: self }
    }
}

impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwmutex.value.get() }
    }
}

impl<T> Deref for WriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwmutx.value.get() }
    }
}

impl<T> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.rwmutx.value.get() }
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        if self.rwmutex.state.fetch_sub(1, Ordering::Release) == 1 {
            self.rwmutex
                .write_wake_counter
                .fetch_add(1, Ordering::Release);
            wake_one(&self.rwmutex.write_wake_counter);
        }
    }
}

impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.rwmutx.state.store(0, Ordering::Release);
        self.rwmutx
            .write_wake_counter
            .fetch_sub(1, Ordering::Release);
        // Wake up one waiting writer.
        wake_one(&self.rwmutx.write_wake_counter);
        // Wake up all waiting readers.
        wake_all(&self.rwmutx.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remutex_should_work() {
        let rw = RwMutex::new(0);
        {
            let rg = rw.read();
            assert_eq!(*rg, 0);

            let rg2 = rw.read();
            assert_eq!(*rg2, 0);
        }

        let mut wg = rw.write();
        *wg += 1;

        drop(wg);

        let rg3 = rw.read();
        assert_eq!(*rg3, 1)
    }
}
