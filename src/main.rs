mod benchmark_report;
mod json;

use std::io::Read;
use std::path::PathBuf;

use ris_error::prelude::*;

use crate::benchmark_report::Report;

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
    bench(arg).map_err(|e| e.to_string())?;

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
    //run_command(format!("cargo bench {}", arg), None)?;

    // collect benchmark groups
    eprintln!("collect benchmark_groups...");

    let source_code = std::fs::read_to_string(source_filepath)?;

    let line = source_code.lines()
        .filter(|line| line.contains("criterion_group!"))
        .next()
        .ris_expect("source code to define a criterion group")?;

    let groups = line.split(',')
        .skip(1)
        .map(|x| x
            .trim()
            .trim_end_matches(';').trim_end()
            .trim_end_matches(')').trim_end()
        )
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
        for function_dir in function_dirs.iter() {
            let report = Report::deserialize(function_dir)?;
            eprintln!("report {}: \n{:#?}\n", function_dir.display(), report);
        }
    }

    Ok(())
}

//==============================================================================
// run command
//==============================================================================
fn run_command(
    cmd: impl AsRef<str>,
    stdout: Option<&mut String>,
) -> RisResult<std::process::ExitStatus> {
    let cmd = cmd.as_ref();
    let splits = cmd.split(' ').map(|x| x.trim()).collect::<Vec<_>>();
    if splits.is_empty() {
        return ris_error::new_result!("cannot run empty cmd")
    }

    let mut command = std::process::Command::new(splits[0]);
    let mut complete_arg = String::new();

    for arg in &splits[1..] {
        let trimmed_arg = arg.trim();
        let is_combined_arg = !complete_arg.is_empty() || trimmed_arg.starts_with("\"");
        if !is_combined_arg {
            command.arg(trimmed_arg);
            continue;
        }

        complete_arg.push(' ');
        complete_arg.push_str(trimmed_arg);

        let combined_arg_is_done = trimmed_arg.ends_with("\"") && !trimmed_arg.ends_with("\\\""); // an escaped `"` does not end the combined arg

        if combined_arg_is_done {
            let trimmed = complete_arg.trim();
            let sub = &trimmed[1..(trimmed.len() - 1)];
            let cleaned = sub.replace("\\\"", "\"");
            command.arg(cleaned);
            complete_arg = String::new(); // reset complete_arg
        }
    }

    if !complete_arg.is_empty() {
        return ris_error::new_result!("syntax error: failed to find closing quotation mark");
    }

    if stdout.is_some() {
        command.stdout(std::process::Stdio::piped());
    }


    eprintln!("running `{}`...", cmd);

    let mut process = command.spawn()?;
    if let Some(stdout_string) = stdout {
        let process_stdout = match process.stdout.as_mut() {
            Some(stdout) => stdout,
            None => return ris_error::new_result!("expect stdout to be Some"),
        };
        process_stdout.read_to_string(stdout_string)?;
    }
    let exit_status = process.wait()?;

    match exit_status.code() {
        Some(code) => eprintln!("`{}` finished with exit code {}", cmd, code),
        None => eprintln!("`{}` finished with no exit code", cmd),
    }

    Ok(exit_status)
}
