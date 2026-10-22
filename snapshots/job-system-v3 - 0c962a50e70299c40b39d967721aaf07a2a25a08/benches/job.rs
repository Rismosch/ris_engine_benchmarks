use criterion::Criterion;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;

use ris_async::thread_pool::StallStrategy;
use ris_async::thread_pool::ThreadPool;
use ris_async::thread_pool::ThreadPoolCreateInfo;
use ris_rng::rng::Rng;
use ris_rng::rng::Seed;

fn job(c: &mut Criterion) {
    let mut group = c.benchmark_group("job_new_job_system");

    let mut rng = Rng::new(Seed::new());

    let hash_iterations = 1_000;
    let hash_input_count = 1_000;
    let mut hash_inputs = Vec::with_capacity(hash_input_count);
    for _ in 0..hash_inputs.capacity() {
        let hash_input = rng.next_u64();
        hash_inputs.push(hash_input);
    }

    let cpu_count = sdl2::cpuinfo::cpu_count() as usize;

    // control
    group.bench_function("control", |b| {
        b.iter(|| {
            for &input in &hash_inputs {
                let result = dummy_work(input, hash_iterations);
                black_box(result);
            }
        });
    });

    // async thread pool
    let bools = [0, 1];
    let strategies = [1, 2, 3];
    for threads in 1..=cpu_count {
        for set_affinity in bools {
            for strategy in strategies {
                let create_info = ThreadPoolCreateInfo {
                    cpu_count,
                    threads,
                    set_affinity: set_affinity == 1,
                    stall_strategy: match strategy {
                        1 => StallStrategy::SpinLoop,
                        2 => StallStrategy::Yield,
                        3 => StallStrategy::Parking,
                        _ => unreachable!(),
                    },
                };
                let pool = ThreadPool::new(create_info).unwrap();

                group.bench_function(
                    format!(
                        "threads: {:02}, aff:{}, str:{} ",
                        threads, set_affinity, strategy
                    ),
                    |b| {
                        b.iter(|| {
                            let worker = ThreadPool::worker().unwrap();
                            for &input in &hash_inputs {
                                worker.submit(move || {
                                    let result = dummy_work(input, hash_iterations);
                                    black_box(result);
                                });
                            }

                            loop {
                                let job_was_run = worker.run_pending_job();
                                if !job_was_run {
                                    break;
                                }
                            }
                        });
                    },
                );

                drop(pool)
            }
        }
    }
}

fn dummy_work(input: u64, iterations: usize) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    let mut hash = input;
    for _ in 0..iterations {
        let mut hasher = DefaultHasher::new();
        hash.hash(&mut hasher);
        hash = hasher.finish();
    }

    hash
}

criterion_group!(benches, job);
criterion_main!(benches);
