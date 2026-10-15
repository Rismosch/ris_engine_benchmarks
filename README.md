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

It appears that this benchmarking tool has a major flaw: When it runs `cargo bench` multiple times (2 or more times), then the following benches will be significantly slower. This makes the results of this tool untrustworthy.

I made this repo to compare my new and old asset system. The old one stored structured data in the compiled assets, which need to be deserialized. The new one stores structs in C-layout, requiring only a copy.

When benched manually, on an Arch Linux x86_64 machine (CPU: 12th Gen Intel i5-1235U (12) @ 4.400GHz, GPU: Intel Alder Lake-UP3 GT2 \[UHD Graphics\], Memory: 7615MiB / 15667MiB), I get the following results (the number is the mean result produced by criterion):

    old: 6.2779 ms
    new: 6.0777 ms

This makes the new asset system ~3% faster, which I guess is a win. But when run through the automatic tool, the effect disappears: The second bench (no matter if old or new) is consistently slower (about ~2 ms slower than when run manually).

My conclusion is that the new asset system is indeed faster, but the automatic tool produces an order effect, which skews results. As such, Rust appears to have been the wrong choice to write this automatic bench tool. A bash script may be the better choice.

Also, the entire engine is initialized instead of just the asset system. This may also influence performance. Especially because currently I assume that the current async job system is a tiny bit slower than the old thread based job system.
