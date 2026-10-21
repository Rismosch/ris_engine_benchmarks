// original C++ implementations of these spinlocks:
// https://david.alvarezrosa.com/posts/optimizing-a-spin-lock/

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkGroup;

pub trait Spinlock : Sync + Send {
    fn new() -> Self;
    fn lock(&self);
    fn unlock(&self);
}

struct SpinlockV1(AtomicBool);
struct SpinlockV2(AtomicBool);
struct SpinlockV3(AtomicBool);
struct SpinlockV4(AtomicBool);
#[repr(align(128))]
struct SpinlockVA(SpinlockV4);

impl Spinlock for SpinlockV1 {
    fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn lock(&self) {
        while self.0.swap(true, Ordering::SeqCst) {}
    }

    fn unlock(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl Spinlock for SpinlockV2 {
    fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn lock(&self) {
        while self.0.swap(true, Ordering::Acquire) {
            std::hint::spin_loop();
        }
    }

    fn unlock(&self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Spinlock for SpinlockV3 {
    fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn lock(&self) {
        while self.0.swap(true, Ordering::Acquire) {
            while self.0.load(Ordering::Relaxed) {
                std::thread::yield_now();
            }
        }
    }

    fn unlock(&self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Spinlock for SpinlockV4 {
    fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn lock(&self) {
        let mut backoff = 1u8;
        while self.0.swap(true, Ordering::Acquire) {
            loop {
                for _ in 0..backoff {
                    std::thread::yield_now();
                }
                backoff = if backoff < 64 {
                    backoff << 1
                } else {
                    64
                };

                if self.0.load(Ordering::Relaxed) {
                    continue;
                } else {
                    break;
                }
            }
        }
    }

    fn unlock(&self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Spinlock for SpinlockVA {
    fn new() -> Self {
        Self(SpinlockV4::new())
    }

    fn lock(&self) {
        self.0.lock();
    }

    fn unlock(&self) {
        self.0.unlock();
    }
}

// bench
fn spinlock(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!("spinlock"));

    run_bench_1_2_4::<SpinlockV1>(&mut group);
    run_bench_1_2_4::<SpinlockV2>(&mut group);
    run_bench_1_2_4::<SpinlockV3>(&mut group);
    run_bench_1_2_4::<SpinlockV4>(&mut group);
    run_bench_1_2_4::<SpinlockVA>(&mut group);
    run_bench_1_2_4_mutex(&mut group);
}

// single bench functions
fn run_bench_1_2_4<TSpinlock> (
    group: &mut BenchmarkGroup<'_, criterion::measurement::WallTime>,
) where TSpinlock: Spinlock + 'static {
    run_bench::<TSpinlock>(group, 1);
    run_bench::<TSpinlock>(group, 2);
    run_bench::<TSpinlock>(group, 4);
}

fn run_bench_1_2_4_mutex (
    group: &mut BenchmarkGroup<'_, criterion::measurement::WallTime>,
) {
    run_bench_mutex(group, 1);
    run_bench_mutex(group, 2);
    run_bench_mutex(group, 4);
}

fn run_bench<TSpinlock> (
    group: &mut BenchmarkGroup<'_, criterion::measurement::WallTime>,
    thread_count: usize,
) where TSpinlock: Spinlock + 'static {
    let type_name = std::any::type_name::<TSpinlock>();
    let version = type_name.chars().last().unwrap();

    group.bench_function(
        format!(
            "V{} threads: {}",
            version,
            thread_count,
        ),
        |b| {
            b.iter(||{
                let mut threads = Vec::with_capacity(thread_count);
                let lock = Arc::new(TSpinlock::new());
                let counter = Arc::new(ris_ptr::SyncUnsafeCell::new(0));

                for i in 0..thread_count {
                    let lock = lock.clone();
                    let counter = counter.clone();
                    let thread = std::thread::spawn(move ||{
                        ris_async::affinity::set_affinity(&[i]).unwrap();

                        for _ in 0..10_000 {
                            lock.lock();
                            let counter = unsafe {&mut *counter.get()};
                            *counter += 1;
                            lock.unlock();
                        }
                    });

                    threads.push(thread);
                }

                for thread in threads {
                    thread.join().unwrap();
                }

                std::hint::black_box(counter);
            });
        }
    );
}

fn run_bench_mutex (
    group: &mut BenchmarkGroup<'_, criterion::measurement::WallTime>,
    thread_count: usize,
) {
    group.bench_function(
        format!(
            "mutex threads: {}",
            thread_count,
        ),
        |b| {
            b.iter(||{
                let mut threads = Vec::with_capacity(thread_count);
                let lock = Arc::new(Mutex::new(()));
                let counter = Arc::new(ris_ptr::SyncUnsafeCell::new(0));

                for i in 0..thread_count {
                    let lock = lock.clone();
                    let counter = counter.clone();
                    let thread = std::thread::spawn(move ||{
                        ris_async::affinity::set_affinity(&[i]).unwrap();

                        for _ in 0..10_000 {
                            let guard = lock.lock().unwrap();
                            let counter = unsafe {&mut *counter.get()};
                            *counter += 1;
                            drop(guard);
                        }
                    });

                    threads.push(thread);
                }

                for thread in threads {
                    thread.join().unwrap();
                }

                std::hint::black_box(counter);
            });
        }
    );
}

criterion_group!(benches, spinlock);
criterion_main!(benches);
