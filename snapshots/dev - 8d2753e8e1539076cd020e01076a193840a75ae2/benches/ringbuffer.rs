// original C++ implementations of these ring buffers:
// https://david.alvarezrosa.com/posts/optimizing-a-lock-free-ring-buffer/
//
// these ring buffers are for single-producer single-consumer queues (spsc)

use std::cell::UnsafeCell;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::mem::MaybeUninit;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkGroup;

// utility to avoid false sharing. align should be the minimum cache line size
#[repr(align(128))]
struct CachePadded<T>(T);

trait RingBuffer<T: Send> {
    fn new() -> Self;
    fn push(&self, value: T) -> Result<(), T>;
    fn pop(&self) -> Option<T>;
}

struct RingBufferV1<T, const N: usize> {
    buffer: [MaybeUninit<T>; N],
    head: usize,
    tail: usize,
}

unsafe impl<T, const N: usize> Sync for RingBufferV1<T, N> where T: Send {}

//==============================================================================
// Mutex/lock
//==============================================================================
struct RingBufferV2<T, const N: usize> {
    inner: Mutex<RingBufferV1<T, N>>,
}

unsafe impl<T, const N: usize> Sync for RingBufferV2<T, N> where T: Send {}

impl<T: Send, const N: usize> RingBuffer<T> for RingBufferV2<T, N> {
    fn new() -> Self {
        let inner = Mutex::new(RingBufferV1 {
            buffer: [const {MaybeUninit::uninit()}; N],
            head: 0,
            tail: 0,
        });
        Self { inner }
    }

    fn push(&self, value: T) -> Result<(), T> {
        let Ok(mut inner) = self.inner.lock() else {
            return Err(value);
        };

        let old_head = inner.head;
        let mut new_head = old_head + 1;
        if new_head == inner.buffer.len() {
            new_head = 0;
        }
        if new_head == inner.tail {
            return Err(value);
        }
        inner.buffer[old_head].write(value);
        inner.head = new_head;

        Ok(())
    }

    fn pop(&self) -> Option<T> {
        let Ok(mut inner) = self.inner.lock() else {
            return None;
        };

        let old_tail = inner.tail;

        if inner.head == old_tail {
            return None;
        }

        let value = unsafe {inner.buffer[old_tail].assume_init_read()};
        let mut new_tail = old_tail + 1;
        if new_tail == inner.buffer.len() {
            new_tail = 0;
        }
        inner.tail = new_tail;

        Some(value)
    }
}

//==============================================================================
// Spinlock
//==============================================================================
pub struct SpinLock<T> {
    locked: CachePadded<AtomicBool>,
    value: UnsafeCell<T>,
}

pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T: Default> Default for SpinLock<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: CachePadded(AtomicBool::new(false)),
            value: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        let mut backoff = 1;
        while self.locked.0.swap(true, Ordering::Acquire) {
            loop {
                for _ in 0..backoff {
                    std::thread::yield_now();
                }

                backoff = if backoff < 64 {
                    backoff << 1
                } else {
                    64
                };

                if self.locked.0.load(Ordering::Relaxed) {
                    continue;
                } else {
                    break;
                }
            }
        }

        SpinLockGuard { lock: self }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.0.store(false, Ordering::Release);
    }
}

impl<T> std::ops::Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> std::ops::DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.value.get() }
    }
}

unsafe impl<T> Sync for SpinLock<T> where T: Send {}

struct RingBufferVS<T, const N: usize> {
    inner: SpinLock<RingBufferV1<T, N>>,
}

unsafe impl<T, const N: usize> Sync for RingBufferVS<T, N> where T: Send {}

impl<T: Send, const N: usize> RingBuffer<T> for RingBufferVS<T, N> {
    fn new() -> Self {
        let inner = SpinLock::new(RingBufferV1 {
            buffer: [const {MaybeUninit::uninit()}; N],
            head: 0,
            tail: 0,
        });
        Self { inner }
    }

    fn push(&self, value: T) -> Result<(), T> {
        let mut inner = self.inner.lock();

        let old_head = inner.head;
        let mut new_head = old_head + 1;
        if new_head == inner.buffer.len() {
            new_head = 0;
        }
        if new_head == inner.tail {
            return Err(value);
        }
        inner.buffer[old_head].write(value);
        inner.head = new_head;

        Ok(())
    }

    fn pop(&self) -> Option<T> {
        let mut inner = self.inner.lock();

        let old_tail = inner.tail;

        if inner.head == old_tail {
            return None;
        }

        let value = unsafe {inner.buffer[old_tail].assume_init_read()};
        let mut new_tail = old_tail + 1;
        if new_tail == inner.buffer.len() {
            new_tail = 0;
        }
        inner.tail = new_tail;

        Some(value)
    }
}

//==============================================================================
// Lock-free (atomics)
//==============================================================================
struct RingBufferV3<T, const N: usize> {
    buffer: [UnsafeCell<MaybeUninit<T>>; N],
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
}

unsafe impl<T, const N: usize> Sync for RingBufferV3<T, N> where T: Send {}

impl<T: Send, const N: usize> RingBuffer<T> for RingBufferV3<T, N> {
    fn new() -> Self {
        Self {
            buffer: [const {UnsafeCell::new(MaybeUninit::uninit())}; N],
            head: CachePadded(AtomicUsize::new(0)),
            tail: CachePadded(AtomicUsize::new(0)),
        }
    }

    fn push(&self, value: T) -> Result<(), T> {
        let head = self.head.0.load(Ordering::SeqCst);
        let mut next_head = head + 1;
        if next_head == self.buffer.len() {
            next_head = 0;
        }
        if next_head == self.tail.0.load(Ordering::SeqCst) {
            return Err(value);
        }
        unsafe {&mut * self.buffer[head].get()}.write(value);
        self.head.0.store(next_head, Ordering::SeqCst);

        Ok(())
    }

    fn pop(&self) -> Option<T> {
        let tail = self.tail.0.load(Ordering::SeqCst);
        if tail == self.head.0.load(Ordering::SeqCst) {
            return None;
        }

        let value = unsafe {(&*self.buffer[tail].get()).assume_init_read()};
        let mut next_tail = tail + 1;
        if next_tail == self.buffer.len() {
            next_tail = 0;
        }
        self.tail.0.store(next_tail, Ordering::SeqCst);

        Some(value)
    }
}

//==============================================================================
// Lock-free (atomics) + memory order
//==============================================================================
struct RingBufferV4<T, const N: usize> {
    buffer: [UnsafeCell<MaybeUninit<T>>; N],
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
}

unsafe impl<T, const N: usize> Sync for RingBufferV4<T, N> where T: Send {}

impl<T: Send, const N: usize> RingBuffer<T> for RingBufferV4<T, N> {
    fn new() -> Self {
        Self {
            buffer: [const {UnsafeCell::new(MaybeUninit::uninit())}; N],
            head: CachePadded(AtomicUsize::new(0)),
            tail: CachePadded(AtomicUsize::new(0)),
        }
    }

    fn push(&self, value: T) -> Result<(), T> {
        let head = self.head.0.load(Ordering::Relaxed);
        let mut next_head = head + 1;
        if next_head == self.buffer.len() {
            next_head = 0;
        }
        if next_head == self.tail.0.load(Ordering::Acquire) {
            return Err(value);
        }
        unsafe {&mut * self.buffer[head].get()}.write(value);
        self.head.0.store(next_head, Ordering::Release);

        Ok(())
    }

    fn pop(&self) -> Option<T> {
        let tail = self.tail.0.load(Ordering::Relaxed);
        if tail == self.head.0.load(Ordering::Acquire) {
            return None;
        }

        let value = unsafe {(&*self.buffer[tail].get()).assume_init_read()};
        let mut next_tail = tail + 1;
        if next_tail == self.buffer.len() {
            next_tail = 0;
        }
        self.tail.0.store(next_tail, Ordering::Release);

        Some(value)
    }
}

//==============================================================================
// Lock-free (atomics) + memory order + index cache
//==============================================================================
struct RingBufferV5<T, const N: usize> {
    buffer: [UnsafeCell<MaybeUninit<T>>; N],
    head: CachePadded<AtomicUsize>,
    head_cached: CachePadded<UnsafeCell<usize>>,
    tail: CachePadded<AtomicUsize>,
    tail_cached: CachePadded<UnsafeCell<usize>>,
}

unsafe impl<T, const N: usize> Sync for RingBufferV5<T, N> where T: Send {}

impl<T: Send, const N: usize> RingBuffer<T> for RingBufferV5<T, N> {
    fn new() -> Self {
        Self {
            buffer: [const {UnsafeCell::new(MaybeUninit::uninit())}; N],
            head: CachePadded(AtomicUsize::new(0)),
            head_cached: CachePadded(UnsafeCell::new(0)),
            tail: CachePadded(AtomicUsize::new(0)),
            tail_cached: CachePadded(UnsafeCell::new(0)),
        }
    }

    fn push(&self, value: T) -> Result<(), T> {
        let head = self.head.0.load(Ordering::Relaxed);
        let mut next_head = head + 1;
        if next_head == self.buffer.len() {
            next_head = 0;
        }
        unsafe {
            if next_head == *self.tail_cached.0.get() {
                *self.tail_cached.0.get() = self.tail.0.load(Ordering::Acquire);
                if next_head == *self.tail_cached.0.get() {
                    return Err(value);
                }
            }
        }
        unsafe {&mut * self.buffer[head].get()}.write(value);
        self.head.0.store(next_head, Ordering::Release);

        Ok(())
    }

    fn pop(&self) -> Option<T> {
        let tail = self.tail.0.load(Ordering::Relaxed);
        unsafe {
            if tail == *self.head_cached.0.get() {
                *self.head_cached.0.get() = self.head.0.load(Ordering::Acquire);
                if tail == *self.head_cached.0.get() {
                    return None;
                }
            }
        }
        let value = unsafe {(&*self.buffer[tail].get()).assume_init_read()};
        let mut next_tail = tail + 1;
        if next_tail == self.buffer.len() {
            next_tail = 0;
        }
        self.tail.0.store(next_tail, Ordering::Release);

        Some(value)
    }
}

//==============================================================================
// Control
//==============================================================================
struct RingBufferVC<T> {
    sender: std::sync::mpsc::Sender<T>,
    receiver: std::sync::mpsc::Receiver<T>,
}

unsafe impl<T> Sync for RingBufferVC<T> where T: Send {}

impl<T: Send> RingBuffer<T> for RingBufferVC<T> {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {sender, receiver}
    }

    fn push(&self, value: T) -> Result<(), T> {
        self.sender
            .send(value)
            .map_err(|e| e.0)
    }

    fn pop(&self) -> Option<T> {
        self.receiver.recv().ok()
    }
}

// bench
fn ringbuffer(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!("ringbuffer"));

    bench_implementation::<RingBufferV2<usize, 100>>(&mut group);
    bench_implementation::<RingBufferVS<usize, 100>>(&mut group);
    bench_implementation::<RingBufferV3<usize, 100>>(&mut group);
    bench_implementation::<RingBufferV4<usize, 100>>(&mut group);
    bench_implementation::<RingBufferV5<usize, 100>>(&mut group);
    bench_implementation::<RingBufferVC<usize>>(&mut group);
}

// single bench functions
fn bench_implementation<TRingBuffer>(
    group: &mut BenchmarkGroup<'_, criterion::measurement::WallTime>,
) where TRingBuffer: RingBuffer<usize> + Sync + Send + 'static {
    const ITERATIONS: usize = 100;
    let type_name = std::any::type_name::<TRingBuffer>();
    let splits = type_name.split('V').collect::<Vec<_>>();
    let version = splits[1].chars().next().unwrap();

    group.bench_function(format!("V{}",version), |b| {b.iter(||{
        let queue = Arc::new(TRingBuffer::new());

        let consumer = {
            let queue = queue.clone();
            std::thread::spawn(move || {
                ris_async::affinity::set_affinity(&[2]).unwrap();

                for _ in 0..ITERATIONS{
                    loop {
                        let Some(value) = queue.pop() else {
                            continue;
                        };

                        std::hint::black_box(value);
                        break;
                    }
                }
            })
        };

        let producer = {
            let queue = queue.clone();
            std::thread::spawn(move || {
                ris_async::affinity::set_affinity(&[3]).unwrap();

                for i in 0..ITERATIONS {
                    let mut to_push = Err(i);
                    loop {
                        to_push = match to_push {
                            Ok(()) => break,
                            Err(value) => queue.push(value),
                        }
                    }
                }
            })
        };

        std::hint::black_box(producer.join()).unwrap();
        std::hint::black_box(consumer.join()).unwrap();
    });});
}

criterion_group!(benches, ringbuffer);
criterion_main!(benches);
