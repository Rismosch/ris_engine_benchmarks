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
