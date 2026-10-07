use std::fmt::Debug;
use std::fmt::Display;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

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
// Sresult
//==============================================================================
pub type Sresult<T> = Result<T, Serror>;

#[derive(Debug, Clone)]
pub struct Serror(String);

impl Serror {
    fn new_result<T>(message: impl AsRef<str>) -> Sresult<T> {
        Err(Self(message.as_ref().to_string()))
    }
}

impl Display for Serror {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<E: std::error::Error + 'static> From<E> for Serror {
    fn from(value: E) -> Self {
        Self(value.to_string())
    }
}

impl From<Serror> for String {
    fn from(value: Serror) -> Self {
        value.0.clone()
    }
}

pub trait Extensions<T> {
    fn sexpect(self, msg: &str) -> Sresult<T>;
}

impl<T> Extensions<T> for Option<T> {
    fn sexpect(self, msg: &str) -> Sresult<T> {
        self.ok_or("Option was None").sexpect(msg)
    }
}

impl<T, E: std::fmt::Display> Extensions<T> for Result<T, E> {
    fn sexpect(self, msg: &str) -> Sresult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(e) => Serror::new_result(format!("expected {}. error: {}", msg, e)),
        }
    }
}

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
    bench(arg)?;

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

fn bench(arg: impl AsRef<str>) -> Sresult<()> {
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
            return Serror::new_result(&message);
        }
    }
}

//==============================================================================
// Bench Job
//==============================================================================
fn bench_job() -> Sresult<()> {
    println!("job bench");
    Ok(())
}

//==============================================================================
// Bench Normal
//==============================================================================
fn bench_normal(arg: impl AsRef<str>) -> Sresult<()> {
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
        .sexpect("source code to define a criterion group")?;

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
            let report = BenchmarkReport::deserialize(function_dir)?;
            eprintln!("report {}: \n{:#?}\n", function_dir.display(), report);
        }
    }

    Ok(())
}

//==============================================================================
// Benchmark Report
//==============================================================================
#[derive(Debug, Clone)]
struct BenchmarkReport {
    benchmark: (),
    estimates: (),
    raw: (),
    sample: (),
    tukey: (),
}

impl BenchmarkReport {
    fn deserialize(path: impl AsRef<Path>) -> Sresult<Self> {
        let path = path.as_ref();
        let benchmark_filepath = path.join("new").join("benchmark.json");
        let estimates_filepath = path.join("new").join("estimates.json");
        let raw_filepath = path.join("new").join("raw.csv");
        let sample_filepath = path.join("new").join("sample.json");
        let tukey_filepath = path.join("new").join("tukey.json");

        Ok(Self {
            benchmark: (),
            estimates: (),
            raw: (),
            sample: (),
            tukey: (),
        })
    }
}


//==============================================================================
// run command
//==============================================================================
fn run_command(
    cmd: impl AsRef<str>,
    stdout: Option<&mut String>,
) -> Sresult<std::process::ExitStatus> {
    let cmd = cmd.as_ref();
    let splits = cmd.split(' ').map(|x| x.trim()).collect::<Vec<_>>();
    if splits.is_empty() {
        return Serror::new_result("cannot run empty cmd")
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
        return Serror::new_result("syntax error: failed to find closing quotation mark");
    }

    if stdout.is_some() {
        command.stdout(std::process::Stdio::piped());
    }


    eprintln!("running `{}`...", cmd);

    let mut process = command.spawn()?;
    if let Some(stdout_string) = stdout {
        let process_stdout = match process.stdout.as_mut() {
            Some(stdout) => stdout,
            None => return Serror::new_result("expect stdout to be Some"),
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
