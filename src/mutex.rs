use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, Ordering},
};

use atomic_wait::{wait, wake_one};

pub struct Mutex<T> {
    /// 0: unlocked
    /// 1: locked
    state: AtomicU32,
    value: UnsafeCell<T>,
}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            state: AtomicU32::new(0),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        // Set the state to 1: locked.
        while self.state.swap(1, Ordering::Acquire) == 1 {
            // If it was already locked.
            // .. wait, unless the state is no longer 1.
            // .. and, try again to swap.
            wait(&self.state, 1);
        }
        // Swap successfully, means locked.
        MutexGuard { mutex: self }
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
        // Set the state back to 0: unlocked.
        self.mutex.state.store(0, Ordering::Release);
        // Wake up one of the waiting threads, if any.
        //
        // Waking one thread is enough, because even if
        // there are multiple threads waiting, only one
        // of them will be able to claim the lock.
        // The next thread to lock it will wake up another
        // thread when it's done with the lock, and so on.
        wake_one(&self.mutex.state);
    }
}
