use std::io::Read;

use ris_error::prelude::*;

//==============================================================================
// run command
//==============================================================================
pub fn run(
    cmd: impl AsRef<str>,
    stdout: Option<&mut String>,
) -> RisResult<std::process::ExitStatus> {
    let cmd = cmd.as_ref();
    let splits = cmd.split(' ').map(|x| x.trim()).collect::<Vec<_>>();
    if splits.is_empty() {
        return ris_error::new_result!("cannot run empty cmd");
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
