use anyhow::Result;
use std::{
    collections::VecDeque,
    sync::atomic::Ordering,
    sync::{atomic::AtomicUsize, Arc, Condvar, Mutex},
};

/// Shared state between the sender and the receiver.
struct Shared<T> {
    /// The queue of messages.
    queue: Mutex<VecDeque<T>>,
    /// The condition variable to notify the receiver when there is a new message.
    available: Condvar,
    /// The number of senders.
    senders: AtomicUsize,
    /// The number of receivers.
    receivers: AtomicUsize,
}

/// The sender of the channel.
pub struct Sender<T> {
    shared: Arc<Shared<T>>,
}

/// The receiver of the channel.
pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
    cached: VecDeque<T>,
}

impl<T> Sender<T> {
    pub fn send(&self, item: T) -> Result<()> {
        if self.total_receivers() == 0 {
            return Err(anyhow::anyhow!("no receiver"));
        }

        let was_empty = {
            let mut inner = self.shared.queue.lock().unwrap();
            let empty = inner.is_empty();
            inner.push_back(item);
            empty
        };

        if was_empty {
            self.shared.available.notify_one();
        }

        Ok(())
    }

    pub fn total_receivers(&self) -> usize {
        self.shared.receivers.load(Ordering::SeqCst)
    }

    pub fn total_queued_items(&self) -> usize {
        let inner = self.shared.queue.lock().unwrap();
        inner.len()
    }
}

impl<T> Receiver<T> {
    pub fn recv(&mut self) -> Result<T> {
        // fast path
        if let Some(t) = self.cached.pop_front() {
            return Ok(t);
        }

        let mut inner = self.shared.queue.lock().unwrap();
        loop {
            match inner.pop_front() {
                Some(t) => {
                    // if there is still message in the queue, swap the cached and the queue.
                    if !inner.is_empty() {
                        std::mem::swap(&mut self.cached, &mut inner);
                    }
                    return Ok(t);
                }
                None if self.total_senders() == 0 => return Err(anyhow::anyhow!("no sender")),
                None => {
                    inner = self
                        .shared
                        .available
                        // Wait for the sender to send a message,
                        // here it would release the MutexGuard(inner) and wait for notification from Condvar.
                        .wait(inner)
                        .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
                }
            }
        }
    }

    pub fn total_senders(&self) -> usize {
        self.shared.senders.load(Ordering::SeqCst)
    }
}

impl<T> Iterator for Receiver<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv().ok()
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.shared.senders.fetch_add(1, Ordering::AcqRel);
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let old = self.shared.senders.fetch_sub(1, Ordering::AcqRel);

        // If all senders are dropped, notify the receiver to read the remaining messages.
        // If there is no available message, the receiver would get an error.
        if old <= 1 {
            self.shared.available.notify_all();
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.shared.receivers.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Create a new unbounded channel.
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let shared = Shared::default();
    let shared = Arc::new(shared);
    (
        Sender {
            shared: shared.clone(),
        },
        Receiver {
            shared,
            cached: VecDeque::with_capacity(INITIAL_SIZE),
        },
    )
}

const INITIAL_SIZE: usize = 32;
impl<T> Default for Shared<T> {
    fn default() -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(INITIAL_SIZE)),
            available: Condvar::new(),
            senders: AtomicUsize::new(1),
            receivers: AtomicUsize::new(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[test]
    fn channel_should_work() {
        let (s, mut r) = unbounded();
        s.send("hello world!".to_string()).unwrap();
        let msg = r.recv().unwrap();
        assert_eq!(msg, "hello world!");
    }

    #[test]
    fn multi_producer_should_work() {
        let (s, mut r) = unbounded();
        let s1 = s.clone();
        let s2 = s.clone();
        let t = thread::spawn(move || {
            s.send(1).unwrap();
        });
        let t1 = thread::spawn(move || {
            s1.send(2).unwrap();
        });
        let t2 = thread::spawn(move || {
            s2.send(3).unwrap();
        });
        for handle in [t, t1, t2] {
            handle.join().unwrap();
        }
        let mut result = [r.recv().unwrap(), r.recv().unwrap(), r.recv().unwrap()];

        // In this test case, we don't ask to guarantee the order of the messages,
        // so we sort the result before asserting.
        result.sort();
        assert_eq!(result, [1, 2, 3]);
    }

    #[test]
    fn receiver_should_be_blocked_when_no_message_is_available() {
        let (s, r) = unbounded();
        let s1 = s.clone();
        thread::spawn(move || {
            for (idx, i) in r.into_iter().enumerate() {
                assert_eq!(idx, i);
            }
            // If we reach here, it means the receiver is not blocked.
            unreachable!()
        });

        thread::spawn(move || {
            for i in 0..100 {
                s.send(i).unwrap();
            }
        });

        // Give the sender some time to send all the messages.
        thread::sleep(Duration::from_millis(1));

        // Now the receiver should be blocked.

        // Send the messages again, to notify the receiver.
        for i in 100..200 {
            s1.send(i).unwrap();
        }

        // Give the receiver some time to receive the messages.
        thread::sleep(Duration::from_millis(1));

        // If the receiver is notified, it should receive all the messages,
        // and the queue should be empty.
        assert_eq!(s1.total_queued_items(), 0);
    }

    #[test]
    fn last_sender_drop_should_error_when_receive() {
        let (s, mut r) = unbounded();
        let s1 = s.clone();
        let senders = [s, s1];
        let total = senders.len();

        // use and drop the senders.
        for sender in senders {
            thread::spawn(move || {
                sender.send(1).unwrap();
                // sender would be dropped here.
            })
            .join()
            .unwrap();
        }

        // although the senders are dropped,
        // the receiver should still be able to receive the existing messages.
        for _ in 0..total {
            r.recv().unwrap();
        }

        // if it tries to receive more messages, it should error.
        assert!(r.recv().is_err());
    }

    #[test]
    fn receiver_drop_should_error_when_send() {
        let (s1, s2) = {
            let (s, _) = unbounded();
            let s1 = s.clone();
            let s2 = s.clone();
            (s1, s2)
        };

        assert!(s1.send(1).is_err());
        assert!(s2.send(2).is_err());
    }

    #[test]
    fn receiver_shall_be_notified_when_all_senders_exit() {
        let (s, mut r) = unbounded::<usize>();
        let (sender, mut receiver) = unbounded::<usize>();
        let t1 = thread::spawn(move || {
            sender.send(0).unwrap();
            assert!(r.recv().is_err());
        });

        thread::spawn(move || {
            receiver.recv().unwrap();
            drop(s);
        });

        t1.join().unwrap();
    }

    #[test]
    fn channel_fast_path_should_work() {
        let (s, mut r) = unbounded::<usize>();
        for i in 0..10 {
            s.send(i).unwrap();
        }

        assert!(r.cached.is_empty());
        assert_eq!(0, r.recv().unwrap());
        assert_eq!(9, r.cached.len());
        assert_eq!(0, s.total_queued_items());
        for (idx, i) in r.take(9).enumerate() {
            assert_eq!(idx + 1, i);
        }
    }
}
