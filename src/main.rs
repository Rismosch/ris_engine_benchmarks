mod benchmark_report;
mod cli;
mod gnuplot;
mod json;

use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use ris_error::prelude::*;

use crate::benchmark_report::Report;
use crate::gnuplot::Gnuplot;

//==============================================================================
// Debug
//==============================================================================

/// useful for debugging plotting, as benching can take quite a while
const SKIP_BENCH: bool = true;

//==============================================================================
// Constants
//==============================================================================
const CURRENT_SNAPSHOT: &str = "dev - 8d2753e8e1539076cd020e01076a193840a75ae2";

const OLD_ASSET_SNAPSHOT: &str = "old-asset-system - 1e2dcf087094fe6129ae06e9d7afdaac449b1f3e";
const NEW_ASSET_SNAPSHOT: &str = CURRENT_SNAPSHOT;

const OLD_JOB_SNAPSHOT: &str = "old-job-system - 1b3f3a1d7fcaa1fc6ecf3b3bdbd25b0dfc9d882b";
const NEW_JOB_SNAPSHOT: &str = CURRENT_SNAPSHOT;

const ARG_ALL: &str = "all";
const ARG_ASSET: &str = "asset";
const ARG_JOB: &str = "job";
const ARG_MAP: &str = "map";
const ARG_MATH: &str = "math";
const ARG_SINCOS: &str = "sincos";
const ARG_VEC: &str = "vec";

const ALL_ARGS: &[&str] = &[ARG_ASSET, ARG_JOB, ARG_MAP, ARG_MATH, ARG_SINCOS, ARG_VEC];

//==============================================================================
// Harness
//==============================================================================
fn main() -> Result<std::process::ExitCode, String> {
    let raw_args = std::env::args().collect::<Vec<_>>();

    if raw_args.len() != 2 {
        print_help("incorrect usage");
        return Ok(1.into());
    }

    let arg = raw_args[1].trim().to_lowercase();
    run_program(arg).map_err(|e| {
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
    eprintln!("available programs:");
    eprintln!("- all");
    for &arg in ALL_ARGS.iter() {
        eprintln!("- {}", arg);
    }
    eprintln!();
}

fn run_program(arg: impl AsRef<str>) -> RisResult<()> {
    let arg = arg.as_ref();

    match arg {
        _ if arg == ARG_ALL => {
            for arg in ALL_ARGS {
                run_program(arg)?;
            }

            Ok(())
        }
        _ if arg == ARG_ASSET => bench_asset(),
        _ if arg == ARG_JOB => bench_job(),
        _ if arg == ARG_MAP => bench(arg, CURRENT_SNAPSHOT),
        _ if arg == ARG_MATH => bench(arg, CURRENT_SNAPSHOT),
        _ if arg == ARG_SINCOS => bench(arg, CURRENT_SNAPSHOT),
        _ if arg == ARG_VEC => bench(arg, CURRENT_SNAPSHOT),
        _ => {
            let message = format!("unknown arg: \"{}\"", arg);
            print_help(&message);
            ris_error::new_result!("{}", message)
        }
    }
}

//==============================================================================
// Bench Job
//==============================================================================
fn bench_asset() -> RisResult<()> {
    println!("not implemented yet");
    Ok(())
}

//==============================================================================
// Bench Job
//==============================================================================
fn bench_job() -> RisResult<()> {
    // collect paths
    let root_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out_dir = root_dir.join("target").join("plots");

    // bench
    bench("job", OLD_JOB_SNAPSHOT)?;
    bench("job", NEW_JOB_SNAPSHOT)?;

    // collect reports
    let old_group_dir = root_dir
        .join("snapshots")
        .join(OLD_JOB_SNAPSHOT)
        .join("target")
        .join("criterion")
        .join("job_old_job_system");
    let new_group_dir = root_dir
        .join("snapshots")
        .join(NEW_JOB_SNAPSHOT)
        .join("target")
        .join("criterion")
        .join("job_new_job_system");

    eprintln!("collecting reports...");
    eprintln!(" - \"{}\"", old_group_dir.display());
    eprintln!(" - \"{}\"", new_group_dir.display());

    let old_reports = find_reports(&old_group_dir)?;
    let new_reports = find_reports(&new_group_dir)?;

    // filter reports and plot

    // when no affinity is set, more workers produce significantly worse
    // benchmarks. one hypothosis is because the task scheduler schedules
    // workers on already running workers. this leads to workers interrupting
    // each other, producing worse performance
    let reports = new_reports
        .iter()
        .filter(|x| !x.benchmark.function_id.contains("aff:0"))
        .cloned()
        .collect::<Vec<_>>();
    plot(
        "New job system, forced affinity",
        out_dir.join("job_new_job_system_forced_affinity"),
        &reports,
    )?;

    // in theory (and most benchmarking i've done), using all cores produces
    // the best benchmarks, assuming settings have been set correctly. the last
    // of the new reports should have the highest threadcount. find it, and
    // compare it against the control, as well as the old job system
    let last = new_reports.last().ris_expect("at least one report")?;
    let key = last
        .benchmark
        .function_id
        .split(',')
        .next()
        .ris_expect("at least one plit")?;

    let mut reports = Vec::new();
    for report in new_reports.iter() {
        if report.benchmark.function_id.contains("aff:0") {
            // skip obviously bad settings
            continue;
        }

        if report.benchmark.function_id == "control" {
            reports.push(report.clone());
        }

        if report.benchmark.function_id.contains(key) {
            reports.push(report.clone());
        }
    }

    for report in old_reports.iter() {
        reports.push(report.clone());
    }

    plot(
        "Job Systems, compared",
        out_dir.join("job_systems_compared"),
        &reports,
    )?;

    Ok(())
}

//==============================================================================
// Bench Normal
//==============================================================================
fn bench(arg: impl AsRef<str>, snapshot: impl AsRef<str>) -> RisResult<()> {
    let arg = arg.as_ref();
    let snapshot = snapshot.as_ref();

    // collect paths
    let root_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let engine_dir = root_dir.join("snapshots").join(snapshot);

    let source_filepath = engine_dir.join("benches").join(format!("{}.rs", &arg));

    let target_dir = engine_dir.join("target").join("criterion");

    let out_dir = root_dir.join("target").join("plots");

    eprintln!();
    eprintln!("source_filepath: {}", source_filepath.display());
    eprintln!("target_dir:      {}", target_dir.display());
    eprintln!("out_dir:         {}", out_dir.display());
    eprintln!();

    // benchmark
    if !SKIP_BENCH {
        eprintln!("change directory... {}", engine_dir.display());
        std::env::set_current_dir(engine_dir)?;
        cli::run(format!("cargo bench {}", arg), None)?;
    }

    // collect benchmark groups
    eprintln!("collect benchmark_groups...");

    let source_code = std::fs::read_to_string(source_filepath)?;

    let lines = source_code.lines().collect::<Vec<_>>();
    let start_line = lines
        .iter()
        .position(|line| line.contains("criterion_group!"))
        .ris_expect("source code to define a criterion group")?;
    let mut group_string = String::new();
    for line in lines.iter().skip(start_line) {
        group_string.push_str(line);
        if line.trim_end().ends_with(';') {
            break;
        }
    }

    let groups = group_string
        .split(',')
        .skip(1)
        .map(|x| {
            x.trim()
                .trim_end_matches(';')
                .trim_end()
                .trim_end_matches(')')
                .trim_end()
        })
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();

    let s = if groups.len() != 1 { "s" } else { "" };
    eprintln!("found {} group{}:", groups.len(), s);
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

    let ies = if group_dirs.len() != 1 { "ies" } else { "y" };
    eprintln!("found {} matching director{}:", group_dirs.len(), ies);
    for group in group_dirs.iter() {
        eprintln!(" - \"{}\"", group.display());
    }

    // build plots
    eprintln!("build plots...");
    for group_dir in group_dirs.iter() {
        let group_name = group_dir.strip_prefix(&target_dir)?.display();

        std::fs::create_dir_all(&out_dir)?;
        let output_filepath = out_dir.join(group_name.to_string());
        eprintln!("output file: {}", output_filepath.display());

        let reports = find_reports(group_dir)?;
        plot(group_name.to_string(), output_filepath, &reports)?;
    }

    Ok(())
}

//==============================================================================
// Utility
//==============================================================================
fn find_reports(group_dir: impl AsRef<Path>) -> RisResult<Vec<Report>> {
    let group_dir = group_dir.as_ref();

    // find bench functions
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

    let s = if function_dirs.len() != 1 { "s" } else { "" };
    eprintln!("found {} bench function{}:", function_dirs.len(), s);
    for function_dir in function_dirs.iter() {
        eprintln!("- \"{}\"", function_dir.display())
    }

    // read files
    eprintln!("read files...");
    let mut reports = Vec::new();
    for function_dir in function_dirs.iter() {
        let report = Report::deserialize(function_dir)?;
        reports.push(report);
    }

    let s = if reports.len() != 1 { "s" } else { "" };
    eprintln!("found {} report{}", reports.len(), s);

    // sort
    reports.sort_by(|lhs, rhs| lhs.benchmark.full_id.cmp(&rhs.benchmark.full_id));

    Ok(reports)
}

fn plot(
    title: impl AsRef<str>,
    output_filestem: impl AsRef<Path>,
    reports: &[Report],
) -> RisResult<()> {
    let title = title.as_ref();
    let output_filestem = output_filestem.as_ref();

    // gnuplot
    let mut gnuplot = Gnuplot::open()?;

    gnuplot.run(format!(
            "set terminal svg size 1600,800 enhanced font 'Arial,12' name '{}'",
            title
    ))?;
    gnuplot.run(format!("set output '{}.svg'", output_filestem.display()))?;

    gnuplot.run(format!(
        "set title '{}: Violin plot'",
        title.replace("_", "\\_"),
    ))?;
    gnuplot.run("set ylabel 'Average time (ms)'")?;
    gnuplot.run("set xlabel ''")?;
    gnuplot.run("set grid ytics")?;

    gnuplot.run("set xtics rotate by 45 right")?;
    for (i, report) in reports.iter().enumerate() {
        let pre = if i == 0 { "set xtics (" } else { "" };

        let post = if i != reports.len() - 1 { ", \\" } else { ")" };

        let i = i + 1;

        gnuplot.run(format!(
            "{}'{}' {}{}",
            pre,
            report.benchmark.full_id.replace("_", "\\_"),
            i,
            post,
        ))?;
    }

    //gnuplot.run("set logscale y")?;
    gnuplot.run(format!("set xrange [0.5:{}.5]", reports.len()))?;
    //gnuplot.run("set style fill solid 0.5")?;
    gnuplot.run("set key off")?;

    for (i, report) in reports.iter().enumerate() {
        let i = i + 1;

        gnuplot.run(format!("$D{} << EOD", i))?;
        for (x, density) in report.kde(200)? {
            gnuplot.run(format!("{} {}", x, density))?;
        }
        gnuplot.run("EOD")?;
    }

    for (i, _) in reports.iter().enumerate() {
        let i = i + 1;
        gnuplot.run(format!("stats $D{} using 2 nooutput", i))?;
        gnuplot.run(format!("max{} = STATS_max", i))?;
    }

    gnuplot.run("set style line 10 lc rgb '#ff0000' lw 2")?;
    gnuplot.run("width = 0.4")?;
    gnuplot.run("color = '#0065b5'")?;

    for (i, _) in reports.iter().enumerate() {
        let pre = if i == 0 { "plot " } else { "" };

        let i = i + 1;

        gnuplot.run(format!(
            "{}$D{} using ({} + width*$2/max{}):($1/1e6) with filledcurves x1={} lc rgb color, \\",
            pre, i, i, i, i,
        ))?;
        gnuplot.run(format!(
            "'' using ({} - width*$2/max{}):($1/1e6) with filledcurves x1={} lc rgb color, \\",
            i, i, i,
        ))?;
    }

    gnuplot.run("'-' using 1:2:3:4 with vectors nohead ls 10")?;
    for (i, report) in reports.iter().enumerate() {
        let mean = report
            .estimates
            .mean
            .as_ref()
            .ris_expect("mean to exist")?
            .point_estimate
            / 1e6;
        gnuplot.run(format!("{}.88 {} 0.24 0", i, mean))?;
    }
    gnuplot.run("e")?;

    // result.txt
    let result_filepath = format!("{}.txt", output_filestem.display());
    let mut result = std::fs::File::create(result_filepath)?;

    let mut max_width = 0;
    for report in reports.iter() {
        let name = &report.benchmark.full_id;
        max_width = usize::max(max_width, name.len());
    }

    let mut copy = reports.to_vec();
    copy.sort_by(|lhs, rhs| {
        let left = lhs
            .estimates
            .mean
            .as_ref()
            .expect("mean to exist")
            .point_estimate;
        let right = rhs
            .estimates
            .mean
            .as_ref()
            .expect("mean to exist")
            .point_estimate;
        if left < right {
            std::cmp::Ordering::Less
        } else if left > right {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });

    writeln!(&mut result, "Legend:")?;
    writeln!(&mut result, "<full id> [lo mean hi]")?;
    writeln!(&mut result)?;
    writeln!(&mut result, "Data: (sorted by mean)")?;

    for report in copy.iter() {
        let name = &report.benchmark.full_id;

        let estimate = report.estimates.mean.as_ref().ris_expect("mean to exist")?;
        let hi = estimate.confidence_interval.upper_bound / 1e6;
        let lo = estimate.confidence_interval.lower_bound / 1e6;
        let mean = estimate.point_estimate / 1e6;

        let mut whitespace = "".to_string();
        for _ in name.len()..max_width {
            whitespace.push(' ');
        }

        writeln!(
            &mut result,
            "{} {} [{:.2e} ms {:.2e} ms {:.2e} ms]",
            name, whitespace, hi, mean, lo
        )?;
    }

    Ok(())
}
