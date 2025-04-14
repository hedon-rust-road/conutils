use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    in_use: AtomicBool,
    ready: AtomicBool,
}

unsafe impl<T> Sync for Channel<T> where T: Send {}

impl<T> Channel<T> {
    pub fn new() -> Self {
        Self {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            in_use: AtomicBool::new(false),
            ready: AtomicBool::new(false),
        }
    }

    /// Panics when trying to send more than one message.
    pub fn send(&self, message: T) {
        if self.in_use.swap(true, Ordering::Relaxed) {
            panic!("can't send more the one message")
        }
        unsafe {
            // Safety: Only call this once!
            (*self.message.get()).write(message);
        }
        self.ready.store(true, Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    pub fn receive(&self) -> T {
        if !self.ready.swap(false, Ordering::Acquire) {
            panic!("no message available")
        }
        // Safety: We've just checked (and reset) the ready flag
        unsafe { (*self.message.get()).assume_init_read() }
    }
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        // 1. no init
        // 2. init but no use
        if *self.ready.get_mut() {
            unsafe {
                self.message.get_mut().assume_init_drop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[test]
    fn one_thread_should_work() {
        let channel = Channel::new();
        channel.send(1);
        assert!(channel.is_ready());
        assert_eq!(channel.receive(), 1);
    }

    #[test]
    fn multi_threads_should_work() {
        let channel = Channel::new();
        thread::scope(|s| {
            s.spawn(|| {
                let load;
                loop {
                    if channel.is_ready() {
                        assert_eq!(channel.receive(), 1);
                        load = true;
                        break;
                    }
                }
                assert!(load);
            });
            s.spawn(|| {
                thread::sleep(Duration::from_millis(10));
                channel.send(1);
            });
        });
    }
}
