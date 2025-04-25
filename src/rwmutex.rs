use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, Ordering},
};

use atomic_wait::{wait, wake_all, wake_one};

pub struct RwMutex<T> {
    /// The number of read locks times two, plus one if there's a writer waiting.
    /// u32::MAX if write locked.
    ///
    /// This means that readers may acquire the lock when
    /// the state is even, but need to block when odd.
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
            if s % 2 == 0 {
                // Even
                assert!(s != u32::MAX - 2, "too many readers");
                match self
                    .state
                    .compare_exchange(s, s + 2, Ordering::Acquire, Ordering::Relaxed)
                {
                    Ok(_) => return ReadGuard { rwmutex: self },
                    Err(e) => s = e,
                }
            }
            // write locked
            if s % 2 == 1 {
                wait(&self.state, s);
                s = self.state.load(Ordering::Relaxed);
            }
        }
    }

    pub fn write(&self) -> WriteGuard<T> {
        let mut s = self.state.load(Ordering::Relaxed);
        loop {
            // Try to lock if unlocked
            if s <= 1 {
                match self
                    .state
                    .compare_exchange(s, u32::MAX, Ordering::Acquire, Ordering::Relaxed)
                {
                    Ok(_) => return WriteGuard { rwmutx: self },
                    Err(e) => {
                        s = e;
                        continue;
                    }
                }
            }
            // Block new readers, by marking sure the state is odd.
            if s % 2 == 0 {
                match self
                    .state
                    .compare_exchange(s, s + 1, Ordering::Relaxed, Ordering::Relaxed)
                {
                    Ok(_) => {}
                    Err(e) => {
                        s = e;
                        continue;
                    }
                }
            }
            // Wait, if it still locked
            let w = self.write_wake_counter.load(Ordering::Acquire);
            s = self.state.load(Ordering::Relaxed);
            if s >= 2 {
                wait(&self.write_wake_counter, w);
                s = self.state.load(Ordering::Relaxed);
            }
        }
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
        // Decrement the state by 2 to remove one read-lock.
        if self.rwmutex.state.fetch_sub(2, Ordering::Release) == 3 {
            // If we decremented from 3 to 1, that means
            // the RwMutex is now unlocked and there is
            // a waiting write, which we wake up.
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
