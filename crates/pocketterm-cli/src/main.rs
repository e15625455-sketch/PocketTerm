use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(flag) = args.next() else {
        eprintln!("pocketterm: expected -c followed by a command");
        return ExitCode::from(2);
    };
    if flag != "-c" {
        eprintln!("pocketterm: unsupported argument {flag}");
        return ExitCode::from(2);
    }
    let Some(command) = args.next() else {
        eprintln!("pocketterm: expected a command after -c");
        return ExitCode::from(2);
    };
    let shell = if cfg!(target_os = "android") {
        "/system/bin/sh"
    } else {
        "sh"
    };
    match Command::new(shell).arg("-c").arg(command).status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(err) => {
            eprintln!("pocketterm: {err}");
            ExitCode::from(1)
        }
    }
}
