//! Command execution and private workspace file operations for PocketTerm.

mod mirrors;
mod packages;

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

static RUNNING_CHILD: Mutex<Option<u32>> = Mutex::new(None);

/// Sends SIGTERM to the active terminal child process, if one is running.
pub fn cancel_running_process() -> bool {
    let Ok(running) = RUNNING_CHILD.lock() else {
        return false;
    };
    let Some(pid) = *running else {
        return false;
    };
    // SAFETY: pid is the process-group ID assigned to the active terminal child.
    unsafe { libc::kill(-(pid as libc::pid_t), libc::SIGTERM) == 0 }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub current_dir: PathBuf,
    pub exit_code: i32,
}

#[derive(Clone, Debug)]
pub struct Terminal {
    root: PathBuf,
    current_dir: PathBuf,
}

impl Terminal {
    pub fn new(files_dir: impl AsRef<Path>, current_dir: Option<&Path>) -> io::Result<Self> {
        fs::create_dir_all(files_dir.as_ref())?;
        let root = fs::canonicalize(files_dir)?;
        let requested = current_dir.unwrap_or(&root);
        let current_dir = secure_path(&root, &root, requested)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        if !current_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "directory does not exist",
            ));
        }
        Ok(Self { root, current_dir })
    }

    #[must_use]
    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    pub fn execute(&mut self, command: &str) -> CommandResult {
        match self.execute_inner(command) {
            Ok((stdout, stderr, code)) => CommandResult {
                stdout,
                stderr,
                current_dir: self.current_dir.clone(),
                exit_code: code,
            },
            Err((name, error)) => CommandResult {
                stdout: String::new(),
                stderr: format!("{name}: {error}\n"),
                current_dir: self.current_dir.clone(),
                exit_code: 1,
            },
        }
    }

    fn execute_inner(&mut self, command: &str) -> Result<(String, String, i32), (String, String)> {
        let args = tokenize(command).map_err(|error| ("shell".to_owned(), error))?;
        let Some(name) = args.first() else {
            return Ok((String::new(), String::new(), 0));
        };
        let tail = &args[1..];
        if name == "pkg" {
            return self
                .pkg(tail)
                .map(|output| (output, String::new(), 0))
                .map_err(|message| (name.clone(), message));
        }
        let result = match name.as_str() {
            "pwd" => self.pwd(),
            "ls" => self.list(tail),
            "cd" => self.change_dir(tail),
            "mkdir" => self.mkdir(tail),
            "touch" => self.touch(tail),
            "cat" => self.cat(tail),
            "cp" => self.copy(tail),
            "mv" => self.move_path(tail),
            "rm" => self.remove(tail),
            "rmdir" => self.remove_dir(tail),
            "find" => self.find(tail),
            _ => {
                return self
                    .run_external(&args)
                    .map_err(|message| (name.clone(), message));
            }
        };
        result
            .map(|(stdout, code)| (stdout, String::new(), code))
            .map_err(|message| (name.clone(), message))
    }

    fn pkg(&self, args: &[String]) -> Result<String, String> {
        mirrors::MirrorManager::open(&self.root)?.execute(args)
    }

    fn pwd(&self) -> Result<(String, i32), String> {
        let shown = self
            .current_dir
            .strip_prefix(&self.root)
            .unwrap_or(Path::new(""));
        let value = if shown.as_os_str().is_empty() {
            "~".to_owned()
        } else {
            format!("~/{}", shown.display())
        };
        Ok((format!("{value}\n"), 0))
    }

    fn change_dir(&mut self, args: &[String]) -> Result<(String, i32), String> {
        if args.len() > 1 {
            return Err("usage: cd [directory]".into());
        }
        let input = args
            .first()
            .map_or_else(|| PathBuf::from("~"), PathBuf::from);
        let path = secure_path(&self.root, &self.current_dir, &input)?;
        if !path.is_dir() {
            return Err(format!("{}: not a directory", input.display()));
        }
        self.current_dir = path;
        Ok((String::new(), 0))
    }

    fn list(&self, args: &[String]) -> Result<(String, i32), String> {
        if args.len() > 1 {
            return Err("usage: ls [path]".into());
        }
        let input = args
            .first()
            .map_or_else(|| PathBuf::from("."), PathBuf::from);
        let path = secure_path(&self.root, &self.current_dir, &input)?;
        if path.is_file() {
            return Ok((format!("{}\n", input.display()), 0));
        }
        let mut entries = fs::read_dir(path)
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        entries.sort_by_key(|entry| entry.file_name());
        let mut output = String::new();
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let suffix = if entry.file_type().map_err(display_error)?.is_dir() {
                "/"
            } else {
                ""
            };
            output.push_str(&name);
            output.push_str(suffix);
            output.push('\n');
        }
        Ok((output, 0))
    }

    fn mkdir(&self, args: &[String]) -> Result<(String, i32), String> {
        if args.is_empty() {
            return Err("usage: mkdir path...".into());
        }
        for arg in args {
            let path = secure_path(&self.root, &self.current_dir, Path::new(arg))?;
            fs::create_dir(&path).map_err(display_error)?;
        }
        Ok((String::new(), 0))
    }

    fn touch(&self, args: &[String]) -> Result<(String, i32), String> {
        if args.is_empty() {
            return Err("usage: touch path...".into());
        }
        for arg in args {
            let path = secure_path(&self.root, &self.current_dir, Path::new(arg))?;
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(display_error)?;
        }
        Ok((String::new(), 0))
    }

    fn cat(&self, args: &[String]) -> Result<(String, i32), String> {
        if args.is_empty() {
            return Err("usage: cat file...".into());
        }
        let mut output = String::new();
        for arg in args {
            let path = secure_path(&self.root, &self.current_dir, Path::new(arg))?;
            let bytes = fs::read(path).map_err(display_error)?;
            let text =
                String::from_utf8(bytes).map_err(|_| "input is not valid UTF-8".to_owned())?;
            output.push_str(&text);
        }
        Ok((output, 0))
    }

    fn copy(&self, args: &[String]) -> Result<(String, i32), String> {
        let [source, destination] = args else {
            return Err("usage: cp source destination".into());
        };
        let from = secure_path(&self.root, &self.current_dir, Path::new(source))?;
        let mut to = secure_path(&self.root, &self.current_dir, Path::new(destination))?;
        if from.is_dir() {
            return Err("directory copy is not supported".into());
        }
        if to.is_dir() {
            let name = from
                .file_name()
                .ok_or_else(|| "invalid source file".to_owned())?;
            to = to.join(name);
        }
        fs::copy(from, to).map_err(display_error)?;
        Ok((String::new(), 0))
    }

    fn move_path(&mut self, args: &[String]) -> Result<(String, i32), String> {
        let [source, destination] = args else {
            return Err("usage: mv source destination".into());
        };
        let from = secure_path(&self.root, &self.current_dir, Path::new(source))?;
        if from == self.root {
            return Err("cannot move the app workspace root".into());
        }
        let mut to = secure_path(&self.root, &self.current_dir, Path::new(destination))?;
        if to.is_dir() {
            let name = from
                .file_name()
                .ok_or_else(|| "invalid source path".to_owned())?;
            to = to.join(name);
        }
        if from.is_dir() && to.starts_with(&from) {
            return Err("cannot move a directory inside itself".into());
        }
        let new_current = self
            .current_dir
            .strip_prefix(&from)
            .ok()
            .map(|suffix| to.join(suffix));
        fs::rename(from, &to).map_err(display_error)?;
        if let Some(path) = new_current {
            self.current_dir = path;
        }
        Ok((String::new(), 0))
    }

    fn remove(&self, args: &[String]) -> Result<(String, i32), String> {
        if args.is_empty() {
            return Err("usage: rm file...".into());
        }
        for arg in args {
            let path = secure_path(&self.root, &self.current_dir, Path::new(arg))?;
            if path.is_dir() {
                return Err(format!("{}: is a directory", arg));
            }
            fs::remove_file(path).map_err(display_error)?;
        }
        Ok((String::new(), 0))
    }

    fn remove_dir(&self, args: &[String]) -> Result<(String, i32), String> {
        if args.is_empty() {
            return Err("usage: rmdir directory...".into());
        }
        for arg in args {
            let path = secure_path(&self.root, &self.current_dir, Path::new(arg))?;
            fs::remove_dir(path).map_err(display_error)?;
        }
        Ok((String::new(), 0))
    }

    fn find(&self, args: &[String]) -> Result<(String, i32), String> {
        if args.len() > 1 {
            return Err("usage: find [path]".into());
        }
        let start = args
            .first()
            .map_or_else(|| PathBuf::from("."), PathBuf::from);
        let path = secure_path(&self.root, &self.current_dir, &start)?;
        let mut paths = Vec::new();
        collect_paths(&self.root, &path, &mut paths)?;
        paths.sort();
        let mut output = String::new();
        for item in paths {
            output.push_str(
                &item
                    .strip_prefix(&self.root)
                    .unwrap_or(&item)
                    .display()
                    .to_string(),
            );
            output.push('\n');
        }
        Ok((output, 0))
    }

    fn run_external(&self, args: &[String]) -> Result<(String, String, i32), String> {
        let executable = self
            .find_executable(&args[0])
            .ok_or_else(|| "command not found".to_owned())?;
        let metadata = fs::metadata(&executable).map_err(display_error)?;
        if !is_executable(&metadata) {
            return Err("permission denied".into());
        }
        let mut command = if is_elf(&executable).map_err(display_error)? {
            if !is_arm64_elf(&executable).map_err(display_error)? {
                return Err("unsupported ELF architecture".into());
            }
            if cfg!(target_os = "android") && executable.starts_with(&self.root) {
                if !cfg!(target_arch = "aarch64") {
                    return Err("unsupported Android ABI".into());
                }
                let mut child = Command::new("/system/bin/linker64");
                child.arg(&executable);
                child
            } else {
                Command::new(&executable)
            }
        } else if let Some((interpreter, optional_arg)) =
            read_shebang(&executable).map_err(display_error)?
        {
            let interpreter = if interpreter.is_absolute() && interpreter.starts_with(&self.root) {
                secure_path(&self.root, &self.current_dir, &interpreter).map_err(display_error)?
            } else {
                interpreter
            };
            let mut child = if interpreter.starts_with(&self.root)
                && is_elf(&interpreter).map_err(display_error)?
            {
                if !is_arm64_elf(&interpreter).map_err(display_error)? {
                    return Err("unsupported ELF architecture".into());
                }
                let mut child = Command::new("/system/bin/linker64");
                child.arg(interpreter);
                child
            } else {
                Command::new(interpreter)
            };
            if let Some(option) = optional_arg {
                child.arg(option);
            }
            child.arg(&executable);
            child
        } else {
            return Err("file is not an executable ELF or supported script".into());
        };
        if !metadata.is_file() {
            return Err("not a regular file".into());
        }
        command.args(&args[1..]).current_dir(&self.current_dir);
        let prefix = self.root.join("usr");
        let temp = prefix.join("tmp");
        fs::create_dir_all(&temp).map_err(display_error)?;
        command
            .env("PREFIX", &prefix)
            .env("POCKETTERM_FILES_DIR", &self.root)
            .env("HOME", &self.root)
            .env("TMPDIR", temp)
            .env(
                "PATH",
                format!("{}:/system/bin", prefix.join("bin").display()),
            )
            .env("LD_LIBRARY_PATH", prefix.join("lib"))
            .env("TERM", "dumb")
            .env("LANG", "C.UTF-8")
            .env_remove("LD_PRELOAD");
        let shim = prefix.join("lib/libpt-exec.so");
        if shim.is_file() {
            command.env("LD_PRELOAD", shim);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let child = command.spawn().map_err(display_error)?;
        let child_id = child.id();
        if let Ok(mut running) = RUNNING_CHILD.lock() {
            *running = Some(child_id);
        }
        let result = child.wait_with_output().map_err(display_error);
        if let Ok(mut running) = RUNNING_CHILD.lock()
            && *running == Some(child_id)
        {
            *running = None;
        }
        let result = result?;
        let stdout = String::from_utf8_lossy(&result.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&result.stderr).into_owned();
        Ok((stdout, stderr, result.status.code().unwrap_or(1)))
    }

    fn find_executable(&self, name: &str) -> Option<PathBuf> {
        let requested = Path::new(name);
        if requested.components().count() > 1 || requested.is_absolute() {
            return secure_path(&self.root, &self.current_dir, requested).ok();
        }
        [
            self.root.join("usr/bin").join(name),
            PathBuf::from("/system/bin").join(name),
        ]
        .into_iter()
        .find(|candidate| candidate.is_file())
    }
}

fn secure_path(root: &Path, cwd: &Path, input: &Path) -> Result<PathBuf, String> {
    let input = if input == Path::new("~") {
        root
    } else if let Ok(relative) = input.strip_prefix("~/") {
        &root.join(relative)
    } else {
        input
    };
    let candidate = if input.is_absolute() {
        input.to_path_buf()
    } else {
        cwd.join(input)
    };
    let normalized = if candidate.exists() {
        fs::canonicalize(&candidate).map_err(display_error)?
    } else {
        let parent = candidate
            .parent()
            .ok_or_else(|| "invalid path".to_owned())?;
        let file = candidate
            .file_name()
            .ok_or_else(|| "invalid path".to_owned())?;
        fs::canonicalize(parent).map_err(display_error)?.join(file)
    };
    if !normalized.starts_with(root) {
        return Err("path is outside the app workspace".into());
    }
    Ok(normalized)
}

fn collect_paths(root: &Path, path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(path).map_err(display_error)?;
    for entry in entries {
        let entry = entry.map_err(display_error)?;
        let is_symlink = entry.file_type().map_err(display_error)?.is_symlink();
        let item = fs::canonicalize(entry.path()).map_err(display_error)?;
        if !item.starts_with(root) {
            return Err("path is outside the app workspace".into());
        }
        out.push(item.clone());
        if !is_symlink && item.is_dir() {
            collect_paths(root, &item, out)?;
        }
    }
    Ok(())
}

fn read_prefix(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut bytes = vec![0; limit];
    let count = file.read(&mut bytes)?;
    bytes.truncate(count);
    Ok(bytes)
}

fn is_elf(path: &Path) -> io::Result<bool> {
    Ok(read_prefix(path, 4)?.starts_with(b"\x7fELF"))
}

fn is_arm64_elf(path: &Path) -> io::Result<bool> {
    let bytes = read_prefix(path, 24)?;
    let kind = bytes
        .get(16..18)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes);
    let version = bytes
        .get(20..24)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes);
    Ok(bytes.len() >= 24
        && bytes.starts_with(b"\x7fELF")
        && bytes[4] == 2
        && bytes[5] == 1
        && bytes[6] == 1
        && bytes[18] == 183
        && bytes[19] == 0
        && matches!(kind, Some(2 | 3))
        && version == Some(1))
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    false
}

fn read_shebang(path: &Path) -> io::Result<Option<(PathBuf, Option<String>)>> {
    let bytes = read_prefix(path, 4096)?;
    if !bytes.starts_with(b"#!") {
        return Ok(None);
    }
    let line = bytes
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    let line = std::str::from_utf8(line)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid shebang"))?;
    let mut parts = line[2..].split_whitespace();
    let Some(interpreter) = parts.next() else {
        return Ok(None);
    };
    let arg = parts.next().map(str::to_owned);
    Ok(Some((PathBuf::from(interpreter), arg)))
}

fn tokenize(command: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut started = false;
    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            started = true;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            started = true;
            continue;
        }
        if let Some(open) = quote {
            if ch == open {
                quote = None;
            } else {
                current.push(ch);
            }
            started = true;
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                started = true;
            }
            c if c.is_whitespace() => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            _ => {
                current.push(ch);
                started = true;
            }
        }
    }
    if escaped {
        current.push('\\');
    }
    if quote.is_some() {
        return Err("unterminated quote".into());
    }
    if started {
        args.push(current);
    }
    Ok(args)
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{Terminal, tokenize};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CHILD_PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn workspace() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("pocketterm-core-{}-{serial}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp workspace");
        path
    }

    #[test]
    fn tokenizes_quoted_arguments() {
        assert_eq!(
            tokenize("touch 'hello world'").expect("parse"),
            ["touch", "hello world"]
        );
    }

    #[test]
    fn file_commands_work_in_private_root() {
        let root = workspace();
        let mut terminal = Terminal::new(&root, None).expect("terminal");
        assert_eq!(terminal.execute("mkdir demo").exit_code, 0);
        terminal.execute("cd demo");
        assert_eq!(terminal.execute("pwd").stdout, "~/demo\n");
        assert_eq!(terminal.execute("cd ~").exit_code, 0);
        terminal.execute("cd demo");
        terminal.execute("touch index.html");
        fs::write(root.join("demo/index.html"), "hello").expect("write");
        assert_eq!(terminal.execute("cat index.html").stdout, "hello");
        terminal.execute("mkdir copies");
        terminal.execute("cp index.html copies");
        terminal.execute("mv copies/index.html moved.html");
        assert_eq!(
            terminal.execute("ls").stdout,
            "copies/\nindex.html\nmoved.html\n"
        );
        terminal.execute("rmdir copies");
        terminal.execute("rm moved.html");
        terminal.execute("rm index.html");
        terminal.execute("cd ..");
        terminal.execute("rmdir demo");
        assert!(!root.join("demo").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_parent_traversal_and_absolute_paths_outside_workspace() {
        let root = workspace();
        let mut terminal = Terminal::new(&root, None).expect("terminal");
        let result = terminal.execute("touch ../escape");
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("outside the app workspace"));
        let outside = root
            .parent()
            .expect("temp parent")
            .join("outside-pocketterm");
        let command = format!("touch {}", outside.display());
        assert_eq!(terminal.execute(&command).exit_code, 1);
        assert!(!outside.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_missing_paths_and_directory_as_cat_input() {
        let root = workspace();
        fs::create_dir(root.join("folder")).expect("create folder");
        let mut terminal = Terminal::new(&root, None).expect("terminal");
        assert!(
            terminal
                .execute("cat missing.txt")
                .stderr
                .contains("No such file")
        );
        assert!(
            terminal
                .execute("cat folder")
                .stderr
                .contains("Is a directory")
        );
        assert!(
            terminal
                .execute("ls missing")
                .stderr
                .contains("No such file")
        );
        assert!(
            terminal
                .execute("cd missing")
                .stderr
                .contains("not a directory")
        );
        assert!(
            terminal
                .execute("cp missing.txt copy.txt")
                .stderr
                .contains("No such file")
        );
        assert!(
            terminal
                .execute("mv missing.txt moved.txt")
                .stderr
                .contains("No such file")
        );
        assert!(
            terminal
                .execute("rm missing.txt")
                .stderr
                .contains("No such file")
        );
        assert!(
            terminal
                .execute("rm folder")
                .stderr
                .contains("is a directory")
        );
        assert!(terminal.execute("rmdir folder").exit_code == 0);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn captures_child_stdout_stderr_and_exit_code_separately() {
        let _guard = CHILD_PROCESS_TEST_LOCK
            .lock()
            .expect("serialize child tests");
        use std::os::unix::fs::PermissionsExt;
        let root = workspace();
        let script = root.join("streams.sh");
        fs::write(
            &script,
            "#!/system/bin/sh\nprintf 'stdout marker\n'\nprintf 'stderr marker\n' >&2\nexit 7\n",
        )
        .expect("write script");
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script, permissions).expect("make executable");
        let mut terminal = Terminal::new(&root, None).expect("terminal");
        let result = terminal.execute("./streams.sh");
        assert_eq!(result.stdout, "stdout marker\n");
        assert_eq!(result.stderr, "stderr marker\n");
        assert_eq!(result.exit_code, 7);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_workspace() {
        use std::os::unix::fs::symlink;
        let root = workspace();
        let outside = workspace();
        fs::write(outside.join("secret"), "private").expect("write outside");
        symlink(&outside, root.join("outside-link")).expect("create link");
        let mut terminal = Terminal::new(&root, None).expect("terminal");
        assert!(
            terminal
                .execute("cat outside-link/secret")
                .stderr
                .contains("outside the app workspace")
        );
        assert!(
            terminal
                .execute("find")
                .stderr
                .contains("outside the app workspace")
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn stops_a_running_command_process_group() {
        let _guard = CHILD_PROCESS_TEST_LOCK
            .lock()
            .expect("serialize child tests");
        use std::os::unix::fs::PermissionsExt;
        use std::thread;
        use std::time::Duration;

        let root = workspace();
        let script = root.join("long-running.sh");
        fs::write(&script, "#!/bin/sh\nwhile :; do :; done\n").expect("write script");
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script, permissions).expect("set executable bit");
        let worker_root = root.clone();
        let worker = thread::spawn(move || {
            let mut terminal = Terminal::new(worker_root, None).expect("terminal");
            terminal.execute("./long-running.sh")
        });
        let cancelled = (0..40).any(|_| {
            if super::cancel_running_process() {
                true
            } else {
                thread::sleep(Duration::from_millis(50));
                false
            }
        });
        assert!(cancelled);
        let result = worker.join().expect("join command thread");
        assert_ne!(result.exit_code, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checks_elf_class_endianness_and_machine() {
        let root = workspace();
        let mut header = vec![0; 24];
        header[..4].copy_from_slice(b"\x7fELF");
        header[4] = 2;
        header[5] = 1;
        header[6] = 1;
        header[16..18].copy_from_slice(&3_u16.to_le_bytes());
        header[18..20].copy_from_slice(&183_u16.to_le_bytes());
        header[20..24].copy_from_slice(&1_u32.to_le_bytes());
        fs::write(root.join("arm64.elf"), &header).expect("write ELF header");
        assert!(super::is_arm64_elf(&root.join("arm64.elf")).expect("read ELF"));
        header[18..20].copy_from_slice(&62_u16.to_le_bytes());
        fs::write(root.join("wrong.elf"), &header).expect("write ELF header");
        assert!(!super::is_arm64_elf(&root.join("wrong.elf")).expect("read ELF"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pwd_is_home_and_find_walks_tree() {
        let root = workspace();
        fs::create_dir(root.join("folder")).expect("mkdir");
        fs::write(root.join("folder/item"), "x").expect("write");
        let mut terminal = Terminal::new(&root, None).expect("terminal");
        assert_eq!(terminal.execute("pwd").stdout, "~\n");
        assert_eq!(terminal.execute("find").stdout, "folder\nfolder/item\n");
        let _ = fs::remove_dir_all(root);
    }
}
