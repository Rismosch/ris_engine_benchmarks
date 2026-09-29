use std::fmt::Debug;
use std::fmt::Display;

//==============================================================================
// Constants
//==============================================================================
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
type Sresult<T> = Result<T, Serror>;

#[derive(Debug, Clone)]
struct Serror(String);

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
    println!("normal bench \"{}\"", arg.as_ref());
    Ok(())
}
