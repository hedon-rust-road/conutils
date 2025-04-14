use std::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
    thread::{self, Thread},
};

pub struct Sender<'a, T> {
    channel: &'a Channel<T>,
    receiving_thread: Thread,
}

pub struct Receiver<'a, T> {
    channel: &'a Channel<T>,
    _no_send: PhantomData<*const ()>,
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
        (
            Sender {
                channel: self,
                receiving_thread: thread::current(),
            },
            Receiver {
                channel: self,
                _no_send: PhantomData,
            },
        )
    }
}

impl<T> Sender<'_, T> {
    /// This never panics. :)
    pub fn send(self, message: T) {
        unsafe { (*self.channel.message.get()).write(message) };
        self.channel.ready.store(true, Ordering::Relaxed);
        self.receiving_thread.unpark();
    }
}

impl<T> Receiver<'_, T> {
    pub fn receive(self) -> T {
        if !self.channel.ready.load(Ordering::Acquire) {
            thread::park();
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
        assert_eq!(receiver.receive(), 1);
    }

    #[test]
    fn multi_threads_should_work() {
        let mut channel = Channel::new();
        let (sender, receiver) = channel.split();
        thread::scope(|s| {
            s.spawn(|| {
                thread::sleep(Duration::from_millis(10));
                sender.send(1);
            });
        });
        assert_eq!(receiver.receive(), 1);
    }
}
