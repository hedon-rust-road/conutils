use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
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

    #[allow(clippy::mut_from_ref)]
    pub fn lock(&self) -> &mut T {
        while self.locked.swap(true, Ordering::Acquire) {
            std::hint::spin_loop();
        }
        unsafe { &mut *self.value.get() }
    }

    /// # Safety
    ///
    /// The &mut T from lock() must be gone!
    /// (And no cheating by keeping reference to fields of that T around!)
    pub unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn one_thread_should_work() {
        let spinlock = SpinLock::new(0);
        let a = spinlock.lock();
        *a = 1;
        unsafe { spinlock.unlock() };
        let b = spinlock.lock();
        assert_eq!(*b, 1);
        unsafe { spinlock.unlock() };
    }

    #[test]
    fn multi_threads_should_work() {
        let spinlock = SpinLock::new(0);

        thread::scope(|s| {
            s.spawn(|| {
                let a = spinlock.lock();
                *a += 1;
                unsafe { spinlock.unlock() };
            });
            s.spawn(|| {
                let a = spinlock.lock();
                *a += 1;
                unsafe { spinlock.unlock() };
            });
        });

        let b = spinlock.lock();
        assert_eq!(*b, 2);
        unsafe { spinlock.unlock() };
    }
}
