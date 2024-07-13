use anyhow::{Context, Result};
use std::env;
use std::fs;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::{
    io::{Stderr, Stdin, Stdout},
    path::PathBuf,
    process::exit,
};

fn main() {
    let mut shell = Shell::new();
    let path = match env::var("PATH") {
        Ok(p) => p.split(':').map(PathBuf::from).collect(),
        Err(_) => Vec::default(),
    };
    loop {
        let input = shell.read_stdin().unwrap();
        let output = parse(input, &path);
        match output {
            Ok(out) => {
                if !out.is_empty() {
                    let _ = shell.write_stdout(out.to_string());
                };
            }
            Err(e) => shell.write_stderr(e.to_string()).unwrap(),
        }
    }
}

struct Shell {
    stdin: Stdin,
    stdout: Stdout,
    stderr: Stderr,
}

impl Shell {
    pub fn new() -> Self {
        io::stdout().flush().unwrap();
        let stdin = io::stdin();
        let stdout = io::stdout();
        let stderr = io::stderr();
        Self {
            stdin,
            stdout,
            stderr,
        }
    }
    pub fn read_stdin(&mut self) -> Result<String> {
        self.stdout
            .write("$ ".as_bytes())
            .context("failed writing shell prompt to stdout.")?;
        self.stdout.flush().context("Failed to flush stdout")?;
        let mut input = String::new();
        self.stdin
            .read_line(&mut input)
            .context("Failed reading from stdin.")?;
        Ok(input)
    }
    pub fn write_stdout(&mut self, text: String) -> Result<()> {
        writeln!(self.stdout, "{}", text)
            .with_context(|| format!("Failed writing message: '{}' to stdout.", text))?;
        self.stdout.flush().context("Failed to flush stdout")?;
        Ok(())
    }
    pub fn write_stderr(&mut self, text: String) -> Result<()> {
        writeln!(self.stdout, "{}", text)
            .with_context(|| format!("Failed writing message: '{}' to stderr.", text))?;
        Ok(())
    }
    pub fn change_dir(path: &PathBuf) -> io::Result<String> {
        env::current_dir()?.push(path);
        env::set_current_dir(path)?;
        Ok(String::default())
    }
}

/// The enum contains Built-in Commands callable in the shell.
/// The Commands are never instantiated and serve only for documentation purposes
/// and to group command related functions.
#[allow(dead_code)]
enum Command {
    Echo,
    Type,
    Exit,
    Pwd,
    Cd,
}

impl Command {
    pub fn is_builtin(command: &str) -> Option<String> {
        let answer = match command {
            "echo" => "echo is a shell builtin",
            "type" => "type is a shell builtin",
            "exit" => "exit is a shell builtin",
            "pwd" => "pwd is a shell builtin",
            "cd" => "cd is a shell builtin",
            _ => return None,
        };
        Some(answer.to_string())
    }
    pub fn get_command_path(command: &str, paths: &[PathBuf]) -> Option<String> {
        paths.iter().find_map(|p| {
            let full_path = p.join(command);
            if (full_path.is_file() && is_executable(&full_path)) {
                Some(full_path.to_str()?.to_string())
            } else {
                None
            }
        })
    }
}

fn parse(input: String, path: &[PathBuf]) -> Result<String> {
    let mut args = input.split_whitespace();
    let command = args.next();
    if command.is_none() {
        return Ok(String::default());
    }
    let command = command.expect("Command was checked for none right beforehand.");

    let response = match command {
        "echo" => args.collect::<Vec<&str>>().join(" "),
        "exit" => exit(0),
        "type" => args.next().map_or_else(
            || "type: expected an argument of a command name".to_string(),
            |cmd| {
                if let Some(builtin) = Command::is_builtin(cmd) {
                    builtin.to_string()
                } else if let Some(path) = Command::get_command_path(cmd, path) {
                    format!("{} is {}", cmd, path)
                } else {
                    format!("{}: not found", cmd)
                }
            },
        ),
        "pwd" => std::env::current_dir().unwrap().display().to_string(),
        "cd" => {
            let target_path = args
                .next()
                .map(|p| p.replace('~', &env::var("HOME").unwrap()))
                .unwrap_or_else(|| env::var("HOME").unwrap_or_default());
            let path_buf = PathBuf::from(target_path);

            if path_buf.as_os_str().is_empty() {
                return Ok("cd: HOME environment variable not set".to_string());
            }

            Shell::change_dir(&path_buf).map_or_else(
                |e| match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        format!("cd: {}: No such file or directory", path_buf.display())
                    }
                    std::io::ErrorKind::PermissionDenied => {
                        format!("cd: permission denied: {}", path_buf.display())
                    }
                    _ => format!("cd: error changing to {}: {}", path_buf.display(), e),
                },
                |_| String::default(),
            )
        }

        otherwise => Command::get_command_path(otherwise, path)
            .map(|path| {
                let output = std::process::Command::new(path)
                    .args(args)
                    .output()
                    .expect("Failed to execute command");
                String::from_utf8_lossy(&output.stdout)
                    .to_string()
                    .trim_end()
                    .to_string()
            })
            .unwrap_or_else(|| format!("{}: command not found", &command)),
    };

    Ok(response)
}

fn is_executable(path: &PathBuf) -> bool {
    fs::metadata(path).map(|metadata| {
        metadata.permissions().mode() & 0o111 != 0
    }).unwrap_or(false)
}
