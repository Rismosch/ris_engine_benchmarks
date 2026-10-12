mod benchmark_report;
mod cli;
mod gnuplot;
mod json;

use core::f64;
use std::os::unix::process;
use std::path::PathBuf;

use ris_error::prelude::*;

use crate::benchmark_report::Report;
use crate::gnuplot::Gnuplot;

//==============================================================================
// Constants
//==============================================================================
const ENGINE_CURRENT_SNAPSHOT: &str = "dev - 8d2753e8e1539076cd020e01076a193840a75ae2";

const ARG_ALL: &str = "all";
const ARG_JOB: &str = "job";
const ARG_MAP: &str = "map";
const ARG_MATH: &str = "math";
const ARG_SINCOS: &str = "sincos";
const ARG_VEC: &str = "vec";

const ALL_ARGS: &[&str] = &[
    ARG_JOB,
    ARG_MAP,
    ARG_MATH,
    ARG_SINCOS,
    ARG_VEC,
];

//==============================================================================
// Harness
//==============================================================================
fn main() -> Result<std::process::ExitCode, String>{
    let raw_args = std::env::args().collect::<Vec<_>>();

    if raw_args.len() != 2 {
        print_help("incorrect usage");
        return Ok(1.into());
    }

    let arg = raw_args[1].trim().to_lowercase();
    bench(arg).map_err(|e| {
        eprintln!("{:?}", e);
        format!("{}", e)
    })?;

    Ok(0.into())
}

fn print_help(message: impl AsRef<str>) {
    let message = message.as_ref();
    eprintln!("{}", message);
    eprintln!();
    eprintln!("usage: cargo run -- <bench>");
    eprintln!();
    eprintln!("available benches:");
    eprintln!("- all");
    for &arg in ALL_ARGS.iter() {
        eprintln!("- {}", arg);
    }
    eprintln!();
}

fn bench(arg: impl AsRef<str>) -> RisResult<()> {
    let arg = arg.as_ref();

    match arg {
        _ if arg == ARG_ALL => {
            for arg in ALL_ARGS {
                bench(arg)?;
            }

            Ok(())
        },
        _ if arg == ARG_JOB => bench_job(),
        _ if arg == ARG_MAP => bench_normal(arg),
        _ if arg == ARG_MATH => bench_normal(arg),
        _ if arg == ARG_SINCOS => bench_normal(arg),
        _ if arg == ARG_VEC => bench_normal(arg),
        _ => {
            let message = format!("unknown arg: \"{}\"", arg);
            print_help(&message);
            return ris_error::new_result!("{}", message);
        }
    }
}

//==============================================================================
// Bench Job
//==============================================================================
fn bench_job() -> RisResult<()> {
    println!("job bench");
    Ok(())
}

//==============================================================================
// Bench Normal
//==============================================================================
fn bench_normal(arg: impl AsRef<str>) -> RisResult<()> {
    let arg = arg.as_ref();

    // collect paths
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("engine")
        .join(ENGINE_CURRENT_SNAPSHOT);

    let source_filepath = cargo_dir
        .join("benches")
        .join(format!("{}.rs",&arg));

    let target_dir = cargo_dir
        .join("target")
        .join("criterion");


    eprintln!();
    eprintln!("source_filepath: {}", source_filepath.display());
    eprintln!("target_dir:      {}", target_dir.display());
    eprintln!();

    // change dir
    eprintln!("change directory... {}", cargo_dir.display());
    std::env::set_current_dir(cargo_dir)?;

    // benchmark
    //cli::run(format!("cargo bench {}", arg), None)?;

    // collect benchmark groups
    eprintln!("collect benchmark_groups...");

    let source_code = std::fs::read_to_string(source_filepath)?;

    let lines = source_code.lines().collect::<Vec<_>>();
    let start_line = lines.iter()
        .position(|line| line.contains("criterion_group!"))
        .ris_expect("source code to define a criterion group")?;
    let mut group_string = String::new();
    for line in start_line.. {
        let line = &lines[line];
        group_string.push_str(line);
        if line.trim_end().ends_with(';') {
            break;
        }
    }

    let groups = group_string.split(',')
        .skip(1)
        .map(|x| x
            .trim()
            .trim_end_matches(';').trim_end()
            .trim_end_matches(')').trim_end()
        )
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();

    let s = if groups.len() != 1 {
        "s"
    } else {
        ""
    };
    eprintln!("found {} group{}:",groups.len(),s);
    for group in groups.iter() {
        eprintln!(" - \"{}\"", group);
    }

    // collect group directories
    let mut group_dirs = Vec::new();
    
    let entries = std::fs::read_dir(&target_dir)?;
    for entry in entries {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let entry_path = entry.path();

        if !metadata.is_dir() {
            continue;
        }

        let candidate = entry_path.strip_prefix(&target_dir)?;
        for group in groups.iter() {
            if candidate.display().to_string().starts_with(group) {
                group_dirs.push(entry_path.clone());
            }
        }
    }

    let ies = if group_dirs.len() != 1 {
        "ies"
    } else {
        "y"
    };
    eprintln!("found {} matching director{}:", group_dirs.len(), ies);
    for group in group_dirs.iter() {
        eprintln!(" - \"{}\"", group.display());
    }

    // find bench functions
    for (i, group_dir) in group_dirs.iter().enumerate() {
        let group_name = group_dir.strip_prefix(&target_dir)?.display();
        let progress = format!("[{}/{}]", i+1, group_dirs.len());

        eprintln!();
        eprintln!("{} find bench functions for {}... ",progress,group_name);

        let mut function_dirs = Vec::new();

        let entries = std::fs::read_dir(group_dir)?;
        for entry in entries {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let entry_path = entry.path();

            if !metadata.is_dir() {
                continue;
            }

            if entry_path.ends_with("report") {
                continue;
            }

            function_dirs.push(entry_path);
        }

        let s = if function_dirs.len() != 1 {
            "s"
        } else {
            ""
        };
        eprintln!("{} found {} bench function{}:", progress, function_dirs.len(), s);
        for function_dir in function_dirs.iter() {
            eprintln!("{} - \"{}\"", progress, function_dir.display())
        }

        // read files
        eprintln!("{} read files...", progress);
        let mut reports = Vec::new();
        for function_dir in function_dirs.iter() {
            let report = Report::deserialize(function_dir)?;
            reports.push(report);
        }

        let s = if reports.len() != 1 {
            "s"
        } else {
            ""
        };
        eprintln!("{} found {} report{}", progress, reports.len(), s);
        
        // spawn gnuplot
        let mut gnuplot = Gnuplot::open()?;

        // build plots
        eprintln!("{} build plots...", progress);

        gnuplot.run("set terminal pngcairo size 700,500 enhanced")?;
        gnuplot.run("set output '/home/simon/repos/benchmarks/violin.png'")?;

        //gnuplot.run("set samples 200")?;
        //gnuplot.run("set xrange [3.0e7:6.0e7]")?;

        for (i, report) in reports.iter().enumerate() {
            let min = report.raw
                .iter()
                .min_by(|a, b| a.iteration_time().cmp(&b.iteration_time()))
                .ris_expect("the report to not be empty")?;
            let max = report.raw
                .iter()
                .max_by(|a, b| a.iteration_time().cmp(&b.iteration_time()))
                .ris_expect("the report to not be empty")?;
            eprintln!(
                "my found min/max: {:?}/{:?}",
                min.iteration_time(),
                max.iteration_time(),
            );

            let i = i + 1;
            gnuplot.run(format!("set table $D{}", i))?;
            gnuplot.run("plot '-' using 1:(1) smooth kdensity")?;

            for raw in report.raw.iter() {
                gnuplot.run(format!("{}", raw.iteration_time().as_nanos()))?;
            }

            gnuplot.run("e")?;
            gnuplot.run("unset table")?;
        }

        for (i, _) in reports.iter().enumerate() {
            let i = i + 1;
            gnuplot.run(format!("stats $D{} using 2 nooutput", i))?;
            gnuplot.run(format!("max{} = STATS_max", i))?;
        }

        gnuplot.run("width = 0.4")?;

        for (i, _) in reports.iter().enumerate() {
            let pre = if i == 0 {
                "plot "
            } else {
                ""
            };

            let post = if i != reports.len() - 1 {
                ", \\"
            } else {
                ""
            };

            let i = i + 1;

            gnuplot.run(format!(
                "{}$D{} using ({} + width*$2/max{}):1 with filledcurves x1={}, \\", pre, i, i, i, i,
            ))?;
            gnuplot.run(format!(
                "'' using ({} - width*$2/max{}):1 with filledcurves x1={}{}", i, i, i, post,
            ))?;
        }

        // kill gnuplot
        //eprint!("{} ", progress);
        drop(gnuplot);
    }

    Ok(())
}

