use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, Ordering},
};

use atomic_wait::{wait, wake_all, wake_one};

pub struct RWMutex<T> {
    /// The number of readers, or u32::MAX if write-locked.
    state: AtomicU32,
    value: UnsafeCell<T>,
}

pub struct ReadGuard<'a, T> {
    rwmutex: &'a RWMutex<T>,
}

pub struct WriteGuard<'a, T> {
    rwmutx: &'a RWMutex<T>,
}

unsafe impl<T> Sync for RWMutex<T> where T: Send + Sync {}

impl<T> RWMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
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
            // Wake up a waiting writer, if any.
            wake_one(&self.rwmutex.state);
        }
    }
}

impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.rwmutx.state.store(0, Ordering::Release);
        // Wake up all waiting readers and writers.
        wake_all(&self.rwmutx.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remutex_should_work() {
        let rw = RWMutex::new(0);
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
