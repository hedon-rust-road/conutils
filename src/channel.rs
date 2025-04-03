use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
};

pub struct Channel<T> {
    queue: Mutex<VecDeque<T>>,
    item_ready: Condvar,
}

impl<T> Channel<T> {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            item_ready: Condvar::new(),
        }
    }

    pub fn send(&self, message: T) {
        self.queue.lock().unwrap().push_back(message);
        self.item_ready.notify_one();
    }

    pub fn receive(&self) -> T {
        let mut b = self.queue.lock().unwrap();
        loop {
            if let Some(message) = b.pop_front() {
                return message;
            }
            b = self.item_ready.wait(b).unwrap();
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
        let channel = Channel::new();
        channel.send(1);
        assert_eq!(channel.receive(), 1);
    }

    #[test]
    fn multi_threads_should_work() {
        let channel = Channel::new();
        thread::scope(|s| {
            s.spawn(|| {
                assert_eq!(channel.receive(), 1);
            });
            s.spawn(|| {
                thread::sleep(Duration::from_millis(10));
                channel.send(1);
            });
        });
    }
}
