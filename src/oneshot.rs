use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Sender<'a, T> {
    channel: &'a Channel<T>,
}

pub struct Receiver<'a, T> {
    channel: &'a Channel<T>,
}

pub struct Channel<T> {
    message: UnsafeCell<MaybeUninit<T>>,
    ready: AtomicBool,
}

unsafe impl<T> Sync for Channel<T> where T: Send {}

impl<T> Channel<T> {
    pub fn new() -> Self {
        Channel {
            message: UnsafeCell::new(MaybeUninit::uninit()),
            ready: AtomicBool::new(false),
        }
    }

    pub fn split(&'_ mut self) -> (Sender<'_, T>, Receiver<'_, T>) {
        *self = Self::new();
        (Sender { channel: self }, Receiver { channel: self })
    }
}

impl<T> Sender<'_, T> {
    /// This never panics. :)
    pub fn send(self, message: T) {
        unsafe { (*self.channel.message.get()).write(message) };
        self.channel.ready.store(true, Ordering::Relaxed);
    }
}

impl<T> Receiver<'_, T> {
    pub fn is_ready(&self) -> bool {
        self.channel.ready.load(Ordering::Relaxed)
    }

    pub fn receive(self) -> T {
        if !self.channel.ready.load(Ordering::Acquire) {
            panic!("no message available!");
        }
        unsafe { (*self.channel.message.get()).assume_init_read() }
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        if *self.ready.get_mut() {
            unsafe {
                self.message.get_mut().assume_init_drop();
            }
        }
    }
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[test]
    fn one_thread_should_work() {
        let mut channel = Channel::new();
        let (sender, receiver) = channel.split();
        sender.send(1);
        assert!(receiver.is_ready());
        assert_eq!(receiver.receive(), 1);
    }

    #[test]
    fn multi_threads_should_work() {
        let mut channel = Channel::new();
        let (sender, receiver) = channel.split();
        thread::scope(|s| {
            s.spawn(|| {
                let load;
                loop {
                    if receiver.is_ready() {
                        assert_eq!(receiver.receive(), 1);
                        load = true;
                        break;
                    }
                }
                assert!(load);
            });
            s.spawn(|| {
                thread::sleep(Duration::from_millis(10));
                sender.send(1);
            });
        });
    }
}
