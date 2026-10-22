# ris_engine_benchmarks

Benchmarks for ris_engine: https://github.com/Rismosch/ris_engine

# Requirements

- The same as [ris_engine](https://github.com/Rismosch/ris_engine#requirements)
- gnuplot

# Usage

Run `cargo run`. It will handle all benchmarking and plot generation.

# How it works

`./engine` contains various snapshots of the engine, which individually implement benchmarks. This crate runs these, and combines the results in a single report.

**Note to developer** Since the combination of reports is custom built, make sure in each bench `.rs` file, the functions in `criterion_group!()` and bench groups are all prefixed with the file name. I.e. all groups in `math.rs` must start with `math`. The combinator assumes this to be the case.

# WARNING (date: 2026-10-14)

Machine: 
- OS: Arch Linux x86_64
- CPU: 12th Gen Intel i5-1235U (12) @ 4.400GHz
- GPU: Intel Alder Lake-UP3 GT2 \[UHD Graphics\]
- Memory: 7615MiB / 15667MiB

It appears that this benchmarking tool has a major flaw: When it runs `cargo bench` multiple times (2 or more times), then the following benches will be significantly slower. This makes the results of this tool untrustworthy.

I made this repo to compare my new and old asset system. The old one stored structured data in the compiled assets, which need to be deserialized. The new one stores structs in C-layout, requiring only a copy.

When benched manually, I get the following results (the number is the mean result produced by criterion):

    old: 6.2779 ms
    new: 6.0777 ms

This makes the new asset system ~3% faster, which I guess is a win. But when run through the automatic tool, the effect disappears: The second bench (no matter if old or new) is consistently slower (about ~2 ms slower than when run manually).

My conclusion is that the new asset system is indeed faster, but the automatic tool produces an order effect, which skews results. As such, Rust appears to have been the wrong choice to write this automatic bench tool. A bash script may be the better choice.

Also, the entire engine is initialized instead of just the asset system. This may also influence performance. Especially because currently I assume that the current async job system is a tiny bit slower than the old thread based job system.

# Report: Spinlock/Ringbuffer (date: 2026-10-14)

Machine: 
- OS: Arch Linux x86_64
- CPU: 12th Gen Intel i5-1235U (12) @ 4.400GHz
- GPU: Intel Alder Lake-UP3 GT2 \[UHD Graphics\]
- Memory: 7615MiB / 15667MiB

It looks like a more sophisticated spinlock is faster than a naive one. But when tested in a Ringbuffer, the normal std::sync::Mutex beat it. (46.936 µs with mutex, 51.701 µs with spinlock)

As for the single-producer-single-consumer rungbuffers, I failed to reproduce the results of [David Álvarez Rosa](https://david.alvarezrosa.com/posts/optimizing-a-lock-free-ring-buffer/). The naive lock free buffers are slightly slower, while sophisticated implementations are slightly faster. But the differences are small and look like statistical noise to me.

It's worth mentioning that for my job system, I want deque behaviour from a thread safe queue (stack + queue). Considering that a simple ring buffer with a single mutex does the trick, complicated solutions are not justified.

# Report: Job System (date: 2026-10-22)

Machine: 
- OS: Arch Linux x86_64
- CPU: 12th Gen Intel i5-1235U (12) @ 4.400GHz
- GPU: Intel Alder Lake-UP3 GT2 \[UHD Graphics\]
- Memory: 7615MiB / 15667MiB

The new job system (v3) with async remove performs about the same as the previous one (v2). v1 seems to be the fastest, but the difference is almost nonexistant, making me believe in statistical noise. And if it isn't, the difference is so small that I don't care. After the 2nd rewrite, the API is very clean and good to use.

However, now i distinguish between 3 stall strategies: SpinLoop (only marked with `std::hint:spin_loop()`, same behaviour as "no parking" on v2), Yield (which uses `std::thread::yield_now()`) and Parking (uses std::thread::park(), same behaviour as "use parking" in v2).

With no affinity, Yield performed much better than SpinLoop. SpinLoop has huge variance. With affinity set, the variance is gone and they perform about the same. Parking still wins overall, even by a small margin (probably below statistical noise).
