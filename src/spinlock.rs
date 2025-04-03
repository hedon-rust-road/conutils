use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

pub struct Guard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        unsafe { self.lock.unlock() };
    }
}

/// implement `Sync` for `SpinLock<T>` in order to make it shareable across threads
/// And we need T implements `Send` in order to make it movable across threads
/// We don't need T to be `Sync` because we will only allow one thread to access the value at a time
unsafe impl<T> Sync for SpinLock<T> where T: Send {}

impl<T> SpinLock<T> {
    pub fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> Guard<T> {
        while self.locked.swap(true, Ordering::Acquire) {
            std::hint::spin_loop();
        }
        Guard { lock: self }
    }

    /// # Safety
    ///
    /// The &mut T from lock() must be gone!
    /// (And no cheating by keeping reference to fields of that T around!)
    pub unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // # Safety
        //
        // The very existence of this Guard
        // guarantees we've exclusively locked the lock.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // # Safety
        //
        // The very existence of this Guard
        // guarantees we've exclusively locked the lock.
        unsafe { &mut *self.lock.value.get() }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn one_thread_should_work() {
        let spinlock = SpinLock::new(0);
        let mut a = spinlock.lock();
        *a += 1;
        drop(a);
        let b = spinlock.lock();
        assert_eq!(*b, 1);
    }

    #[test]
    fn multi_threads_should_work() {
        let spinlock = SpinLock::new(0);

        thread::scope(|s| {
            s.spawn(|| {
                let mut a = spinlock.lock();
                *a += 1;
            });
            s.spawn(|| {
                let mut a = spinlock.lock();
                *a += 1;
            });
        });

        let b = spinlock.lock();
        assert_eq!(*b, 2);
    }
}
