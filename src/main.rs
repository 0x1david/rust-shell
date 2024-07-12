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
    pub fn get_command_path(command: &str, path: &[PathBuf]) -> Option<String> {
        path.iter().find_map(|p| {
            let mut path = p.to_owned();
            path.push(command);
            if fs::read(&path).is_ok() && is_executable(&path) {
                Some(path.to_str()?.to_string())
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
        "echo" => args.fold(String::new(), |mut acc, s| {
            if !acc.is_empty() {
                acc.push(' ');
            }
            acc.push_str(s.as_ref());
            acc
        }),
        "exit" => exit(0),
        "type" => match args.next() {
            Some(a) => Command::is_builtin(a)
                .or_else(|| Command::get_command_path(a, path).map(|v| format!("{} is {}", a, v)))
                .unwrap_or_else(|| format!("{}: not found", a)),
            None => "type: expected an argument of a command name".to_string(),
        },
        "pwd" => std::env::current_dir().unwrap().display().to_string(),
        "cd" => match args.next() {
            Some(path) => match PathBuf::try_from(path.replace('~', &env::var("HOME")?)) {
                Ok(path_buf) => match Shell::change_dir(&path_buf) {
                    Ok(_) => String::default(),
                    Err(e) => match e.kind() {
                        std::io::ErrorKind::NotFound => { format!("cd: {}: No such file or directory", path_buf.display()) }
                        std::io::ErrorKind::PermissionDenied => { format!("cd: permission denied: {}", path_buf.display()) }
                        _ => format!("cd: error changing to {}: {}", path_buf.display(), e),
                    },
                },
                Err(_) => format!("cd: invalid path '{}'", path),
            },
            None => match env::var("HOME") {
                Ok(home) => match env::set_current_dir(Path::new(&home)) {
                    Ok(_) => String::default(),
                    Err(e) => format!("cd: error changing to home directory: {}", e),
                },
                Err(_) => "cd: HOME environment variable not set".to_string(),
            },
        },

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
    if let Ok(metadata) = fs::metadata(path) {
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        mode & 0o111 != 0
    } else {
        false
    }
}
