use pocketterm_core::Terminal;
use std::path::PathBuf;
use std::process::ExitCode;

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
    let root = std::env::var_os("POCKETTERM_FILES_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut terminal = match Terminal::new(&root, None) {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("pocketterm: {error}");
            return ExitCode::from(1);
        }
    };
    let result = terminal.execute(&command);
    print!("{}", result.stdout);
    eprint!("{}", result.stderr);
    ExitCode::from(result.exit_code.clamp(0, 255) as u8)
}
