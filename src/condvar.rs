use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use atomic_wait::{wait, wake_all, wake_one};

use crate::MutexGuard;

pub struct Condvar {
    counter: AtomicU32,
    num_waiters: AtomicUsize,
}

impl Condvar {
    pub const fn new() -> Self {
        Self {
            counter: AtomicU32::new(0),
            num_waiters: AtomicUsize::new(0),
        }
    }

    pub fn notify_one(&self) {
        if self.num_waiters.load(Ordering::Relaxed) > 0 {
            self.counter.fetch_add(1, Ordering::Relaxed);
            wake_one(&self.counter);
        }
    }

    pub fn notify_all(&self) {
        if self.num_waiters.load(Ordering::Relaxed) > 0 {
            self.counter.fetch_add(1, Ordering::Relaxed);
            wake_all(&self.counter);
        }
    }

    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        self.num_waiters.fetch_add(1, Ordering::Relaxed);
        let counter_value = self.counter.load(Ordering::Relaxed);

        // Unlock the mutex by dropping the guard,
        // but remember the mutex so we can lock it again later.
        let mutex = guard.mutex;
        drop(guard);

        // Wait, but only if the counter hasn't changed since unlocking.
        wait(&self.counter, counter_value);

        self.num_waiters.fetch_sub(1, Ordering::Relaxed);

        // If the condition matches, lock the mutex and do biz logic.
        mutex.lock()
    }
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, thread, time::Duration};

    use crate::Mutex;

    use super::*;

    #[test]
    fn condvar_should_work() {
        let mutex = Mutex::new(0);
        let condvar = Condvar::new();

        let mut wakeups = 0;

        thread::scope(|s| {
            s.spawn(|| {
                thread::sleep(Duration::from_secs(1));
                *mutex.lock() = 123;
                condvar.notify_one();
            });

            let mut m = mutex.lock();
            while *m < 100 {
                m = condvar.wait(m);
                wakeups += 1;
            }

            assert_eq!(*m, 123);
        });

        // Check that the main thread actually did wait(not busy-loop),
        // while still allowing for a few spurious wake ups.
        assert!(0 < wakeups && wakeups < 10);
    }

    #[test]
    fn condvar_usage() {
        let queue = Mutex::new(VecDeque::new());
        let not_empty = Condvar::new();

        thread::scope(|s| {
            s.spawn(|| loop {
                let mut q = queue.lock();
                let item = loop {
                    if let Some(item) = q.pop_front() {
                        break item;
                    } else {
                        q = not_empty.wait(q);
                    }
                };
                drop(q);
                dbg!(item);
                if item == 9 {
                    return;
                }
            });

            for i in 0..10 {
                queue.lock().push_back(i);
                not_empty.notify_one();
                thread::sleep(Duration::from_millis(10));
            }
        });
    }
}
