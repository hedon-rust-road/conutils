use std::{
    cell::UnsafeCell,
    mem::ManuallyDrop,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{fence, AtomicUsize, Ordering},
};

struct ArcData<T> {
    /// Number of `Arc`s.
    data_ref_count: AtomicUsize,
    /// Number of `Weak`s, plus one if there are any `Arc`s.
    alloc_ref_count: AtomicUsize,
    /// The data. Dropped if there are only weak pointers left.
    data: UnsafeCell<ManuallyDrop<T>>,
}

pub struct Arc<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Arc<T> {}
unsafe impl<T: Send + Sync> Sync for Arc<T> {}

pub struct Weak<T> {
    ptr: NonNull<ArcData<T>>,
}

unsafe impl<T: Send + Sync> Send for Weak<T> {}
unsafe impl<T: Send + Sync> Sync for Weak<T> {}

impl<T> Arc<T> {
    pub fn new(data: T) -> Arc<T> {
        Arc {
            ptr: NonNull::from(Box::leak(Box::new(ArcData {
                data_ref_count: AtomicUsize::new(1),
                alloc_ref_count: AtomicUsize::new(1),
                data: UnsafeCell::new(ManuallyDrop::new(data)),
            }))),
        }
    }

    pub fn get_mut(arc: &mut Self) -> Option<&mut T> {
        if arc
            .data()
            .alloc_ref_count
            .compare_exchange(1, usize::MAX, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return None;
        }
        let is_unique = arc.data().data_ref_count.load(Ordering::Relaxed) == 1;
        // Release matches Acquire increment in `downgrade`, to make sure any
        // changes to the data_ref_count that come after `downgrade` don't
        // change the is_unique result above.
        arc.data().alloc_ref_count.store(1, Ordering::Release);
        if !is_unique {
            return None;
        }
        // Acquire to match Arc::drop's Release decrement, to make sure nothing
        // lese is accessing the data.
        fence(Ordering::Acquire);
        unsafe { Some(&mut *arc.data().data.get()) }
    }

    pub fn downgrade(&self) -> Weak<T> {
        let mut n = self.data().alloc_ref_count.load(Ordering::Relaxed);
        loop {
            if n == usize::MAX {
                std::hint::spin_loop();
                n = self.data().alloc_ref_count.load(Ordering::Relaxed);
                continue;
            }
            assert!(n < usize::MAX - 1);

            // Acquire sets happens-before with `get_mut` release-store.
            if let Err(e) = self.data().alloc_ref_count.compare_exchange_weak(
                n,
                n + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                n = e;
                continue;
            }
            return Weak { ptr: self.ptr };
        }
    }

    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: Since there's an Arc to the data,
        // the data exists and may be shared
        unsafe { &*self.data().data.get() }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Self {
        if self.data().data_ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Arc { ptr: self.ptr }
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        if self.data().data_ref_count.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            // Safety: The data reference counter is zero,
            // so nothing will access the data anymore.
            unsafe {
                ManuallyDrop::drop(&mut *self.data().data.get());
            }
            // Now that there's no `Arc<T>`s left,
            // drop the implicit week pointer that represented all `Arc<T>`s.
            drop(Weak { ptr: self.ptr });
        }
    }
}

impl<T> Weak<T> {
    fn data(&self) -> &ArcData<T> {
        unsafe { self.ptr.as_ref() }
    }

    pub fn upgrade(&self) -> Option<Arc<T>> {
        let mut n = self.data().data_ref_count.load(Ordering::Relaxed);
        loop {
            if n == 0 {
                return None;
            }
            assert!(n < usize::MAX);
            if let Err(e) = self.data().data_ref_count.compare_exchange_weak(
                n,
                n + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                n = e;
                continue;
            }
            return Some(Arc { ptr: self.ptr });
        }
    }
}

impl<T> Clone for Weak<T> {
    fn clone(&self) -> Self {
        if self.data().alloc_ref_count.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            std::process::abort();
        }
        Weak { ptr: self.ptr }
    }
}

impl<T> Drop for Weak<T> {
    fn drop(&mut self) {
        if self.data().alloc_ref_count.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            unsafe {
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[test]
    fn arc_should_work() {
        static NUM_DROPS: AtomicUsize = AtomicUsize::new(0);
        struct DetectDrop;

        impl Drop for DetectDrop {
            fn drop(&mut self) {
                NUM_DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Create an Arc with two weak pointers.
        let x = Arc::new(("hello", DetectDrop));
        let y = Arc::downgrade(&x);
        let z = Arc::downgrade(&x);

        let t = std::thread::spawn(move || {
            // Weak pointer should be upgradbale at this point.
            let y = y.upgrade().unwrap();
            assert_eq!(y.0, "hello");
        });
        assert_eq!(x.0, "hello");
        t.join().unwrap();

        // The data shouldn't be dropped yet,
        // and the weak pointer should be upgradable
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 0);
        assert!(z.upgrade().is_some());

        drop(x);

        // Now, the data should be dropped, and the
        // weak pointer should no longer be upgradable.
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 1);
        assert!(z.upgrade().is_none());
    }

    // Helper methods for testing reference counts
    impl<T> Arc<T> {
        fn get_data_ref_count(&self) -> usize {
            self.data().data_ref_count.load(Ordering::Relaxed)
        }

        fn get_alloc_ref_count(&self) -> usize {
            self.data().alloc_ref_count.load(Ordering::Relaxed)
        }
    }

    #[test]
    fn test_basic_arc() {
        // Test creation and basic reference counting
        let x = Arc::new(42);
        assert_eq!(*x, 42);
        assert_eq!(x.get_data_ref_count(), 1);
        assert_eq!(x.get_alloc_ref_count(), 1);

        // Test cloning
        let y = Arc::clone(&x);
        assert_eq!(*y, 42);
        assert_eq!(x.get_data_ref_count(), 2);
        assert_eq!(x.get_alloc_ref_count(), 1);

        // Test dropping
        drop(y);
        assert_eq!(x.get_data_ref_count(), 1);
        assert_eq!(x.get_alloc_ref_count(), 1);
    }

    #[test]
    fn test_weak_reference() {
        let strong = Arc::new(42);
        assert_eq!(strong.get_data_ref_count(), 1);
        assert_eq!(strong.get_alloc_ref_count(), 1);

        let weak = Arc::downgrade(&strong);
        assert_eq!(strong.get_data_ref_count(), 1); // strong count unchanged
        assert_eq!(strong.get_alloc_ref_count(), 2); // alloc count increased

        // Test upgrade succeeds while strong reference exists
        let upgraded = weak.upgrade().unwrap();
        assert_eq!(*upgraded, 42);
        assert_eq!(strong.get_data_ref_count(), 2);
        assert_eq!(strong.get_alloc_ref_count(), 2);

        // Drop all strong references
        drop(upgraded);
        drop(strong);

        // Weak upgrade should now fail
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_multiple_threads() {
        use std::thread;

        let arc = Arc::new(42);
        let arc2 = Arc::clone(&arc);

        let handle = thread::spawn(move || {
            assert_eq!(*arc2, 42);
            let weak = Arc::downgrade(&arc2);
            assert_eq!(weak.upgrade().unwrap().get_data_ref_count(), 3);
        });

        assert_eq!(*arc, 42);
        handle.join().unwrap();

        assert_eq!(arc.get_data_ref_count(), 1);
        assert_eq!(arc.get_alloc_ref_count(), 1);
    }

    #[test]
    fn test_get_mut() {
        let mut x = Arc::new(42);

        // Should be able to get mutable reference when unique
        let value = Arc::get_mut(&mut x).unwrap();
        *value = 43;
        let _ = value;
        assert_eq!(*x, 43);

        // Not other reference, get_mut should still succeed
        let value = Arc::get_mut(&mut x).unwrap();
        *value = 44;
        assert_eq!(*x, 44);

        // If has weak, get_mut should fail
        let weak = Arc::downgrade(&x);
        assert!(Arc::get_mut(&mut x).is_none());
        let _ = weak;
    }

    #[test]
    fn cycle_reference_no_weak_should_not_free_resource() {
        static NUM_DROPS: AtomicUsize = AtomicUsize::new(0);
        struct Node {
            child: RefCell<Vec<Arc<Node>>>,
            parent: RefCell<Option<Arc<Node>>>,
        }
        impl Drop for Node {
            fn drop(&mut self) {
                NUM_DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }
        impl Node {
            pub fn new() -> Self {
                Self {
                    child: RefCell::new(vec![]),
                    parent: RefCell::new(None),
                }
            }
            pub fn add_child(parent: &Arc<Node>, child: Arc<Node>) {
                *child.parent.borrow_mut() = Some(parent.clone());
                parent.child.borrow_mut().push(child);
            }
        }
        {
            let root = Arc::new(Node::new());
            let child1 = Arc::new(Node::new());
            let child2 = Arc::new(Node::new());
            Node::add_child(&root, child1);
            Node::add_child(&root, child2);
        }

        // not equal to 3, means the root/child1/child3 haven't been dropped.
        assert_ne!(NUM_DROPS.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn cycle_reference_use_weak_should_free_resource() {
        static NUM_DROPS: AtomicUsize = AtomicUsize::new(0);
        struct Node {
            child: RefCell<Vec<Arc<Node>>>,
            parent: RefCell<Option<Weak<Node>>>,
        }
        impl Drop for Node {
            fn drop(&mut self) {
                NUM_DROPS.fetch_add(1, Ordering::Relaxed);
            }
        }
        impl Node {
            pub fn new() -> Self {
                Self {
                    child: RefCell::new(vec![]),
                    parent: RefCell::new(None),
                }
            }
            pub fn add_child(parent: &Arc<Node>, child: Arc<Node>) {
                *child.parent.borrow_mut() = Some(parent.downgrade());
                parent.child.borrow_mut().push(child);
            }
        }
        {
            let root = Arc::new(Node::new());
            let child1 = Arc::new(Node::new());
            let child2 = Arc::new(Node::new());
            Node::add_child(&root, child1);
            Node::add_child(&root, child2);
        }

        // equal to 3, means the root/child1/child3 have been dropped.
        assert_eq!(NUM_DROPS.load(Ordering::Relaxed), 3);
    }
}
