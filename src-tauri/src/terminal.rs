use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};
use uuid::Uuid;

#[cfg(target_os = "linux")]
use std::{
    process::{Child, Stdio},
    thread,
    time::{Duration, Instant},
};

fn executable_file(path: &Path) -> Option<PathBuf> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return None;
    }
    fs::canonicalize(path).ok()
}

pub fn find_in_path(name: &str) -> Option<PathBuf> {
    if name.contains(std::path::MAIN_SEPARATOR) {
        return executable_file(Path::new(name));
    }

    if let Some(path) = env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find_map(|candidate| executable_file(&candidate))
    }) {
        return Some(path);
    }

    // Desktop apps do not always inherit the user's interactive PATH.
    ["/usr/local/bin", "/usr/bin", "/snap/bin"]
        .iter()
        .map(|directory| Path::new(directory).join(name))
        .find_map(|candidate| executable_file(&candidate))
}

pub fn validate_executable_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err("Use an absolute path to the Codex executable".to_string());
    }
    executable_file(path)
        .ok_or_else(|| format!("No executable Codex file was found at {}", path.display()))
}

pub fn validate_directory_path(value: &str, label: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(format!("Use an absolute path for {label}"));
    }
    if !path.is_dir() {
        return Err(format!("No folder was found at {}", path.display()));
    }
    fs::canonicalize(path).map_err(|error| format!("Could not open {}: {error}", path.display()))
}

pub fn open_thread(codex: &Path, thread_id: &str, cwd: &str) -> Result<String, String> {
    let id = Uuid::parse_str(thread_id)
        .map_err(|_| "Codex returned an invalid thread id".to_string())?;
    let working_directory = validate_directory_path(cwd, "the thread's working folder")?;

    #[cfg(target_os = "macos")]
    {
        return open_macos(codex, Some(&id.to_string()), &working_directory);
    }

    #[cfg(target_os = "linux")]
    {
        return open_linux(codex, Some(&id.to_string()), &working_directory);
    }

    #[allow(unreachable_code)]
    Err("Terminal handoff is only supported on macOS and Linux".to_string())
}

pub fn start_agent(codex: &Path, cwd: &Path) -> Result<String, String> {
    let working_directory = validate_directory_path(
        &cwd.to_string_lossy(),
        "the selected project's working folder",
    )?;

    #[cfg(target_os = "macos")]
    {
        return open_macos(codex, None, &working_directory);
    }

    #[cfg(target_os = "linux")]
    {
        return open_linux(codex, None, &working_directory);
    }

    #[allow(unreachable_code)]
    Err("Starting an agent is only supported on macOS and Linux".to_string())
}

#[cfg(target_os = "macos")]
fn open_macos(codex: &Path, thread_id: Option<&str>, cwd: &Path) -> Result<String, String> {
    let cwd = cwd.to_string_lossy();
    let (command, action, failure) = match thread_id {
        Some(thread_id) => (
            format!(
                "{} resume {} --remote unix:// --cd {}",
                shell_quote(&codex.to_string_lossy()),
                shell_quote(thread_id),
                shell_quote(&cwd),
            ),
            "Opening the thread in Terminal",
            "resume this Codex thread",
        ),
        None => (
            format!(
                "{} --remote unix:// --cd {}",
                shell_quote(&codex.to_string_lossy()),
                shell_quote(&cwd),
            ),
            "Starting Codex in Terminal",
            "start Codex",
        ),
    };
    let script_path = env::temp_dir().join(format!("plow-terminal-{}.command", Uuid::new_v4()));
    let script = format!(
        "#!/bin/sh\ntrap 'rm -f -- \"$0\"' EXIT\ncd -- {} || exit 1\n{}\nstatus=$?\nif [ \"$status\" -ne 0 ]; then\n  printf '\\nPlow could not {} (exit %s).\\n' \"$status\"\n  printf 'Press Return to close this window. '\n  read -r _\nfi\nexit \"$status\"\n",
        shell_quote(&cwd),
        command,
        failure,
    );
    fs::write(&script_path, script)
        .map_err(|error| format!("Could not prepare terminal handoff: {error}"))?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("Could not make terminal handoff executable: {error}"))?;
    Command::new("/usr/bin/open")
        .args(["-a", "Terminal"])
        .arg(&script_path)
        .spawn()
        .map_err(|error| format!("Could not open Terminal: {error}"))?;
    Ok(action.to_string())
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "linux")]
const LINUX_RESUME_SCRIPT: &str = r#"
if ! cd -- "$3"; then
  printf '\nPlow could not open the thread working folder.\n'
  exec "${SHELL:-/bin/sh}" -l
fi
"$1" resume "$2" --remote unix:// --cd "$3"
status=$?
if [ "$status" -ne 0 ]; then
  printf '\nPlow could not resume this Codex thread (exit %s).\n' "$status"
  printf 'The command was: codex resume %s --remote unix:// --cd %s\n' "$2" "$3"
  exec "${SHELL:-/bin/sh}" -l
fi
exit "$status"
"#;

#[cfg(target_os = "linux")]
const LINUX_START_SCRIPT: &str = r#"
if ! cd -- "$2"; then
  printf '\nPlow could not open the selected project folder.\n'
  exec "${SHELL:-/bin/sh}" -l
fi
"$1" --remote unix:// --cd "$2"
status=$?
if [ "$status" -ne 0 ]; then
  printf '\nPlow could not start Codex (exit %s).\n' "$status"
  printf 'The command was: codex --remote unix:// --cd %s\n' "$2"
  exec "${SHELL:-/bin/sh}" -l
fi
exit "$status"
"#;

#[cfg(target_os = "linux")]
const LINUX_TERMINALS: &[(&str, &[&str])] = &[
    ("gnome-terminal", &["--"]),
    ("kgx", &["--"]),
    ("ptyxis", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["--execute"]),
    ("mate-terminal", &["--"]),
    ("kitty", &["--"]),
    ("wezterm", &["start", "--"]),
    ("foot", &[]),
    ("alacritty", &["-e"]),
    ("tilix", &["-e"]),
    ("lxterminal", &["-e"]),
    ("xterm", &["-e"]),
    ("xdg-terminal-exec", &["--"]),
    ("x-terminal-emulator", &["-e"]),
];

#[cfg(target_os = "linux")]
fn open_linux(codex: &Path, thread_id: Option<&str>, cwd: &Path) -> Result<String, String> {
    open_linux_with_candidates(codex, thread_id, cwd, LINUX_TERMINALS)
}

#[cfg(target_os = "linux")]
fn open_linux_with_candidates(
    codex: &Path,
    thread_id: Option<&str>,
    cwd: &Path,
    candidates: &[(&str, &[&str])],
) -> Result<String, String> {
    for (terminal, prefix) in candidates {
        let Some(path) = find_in_path(terminal) else {
            continue;
        };
        let mut command = Command::new(path);
        command
            .args(*prefix)
            .arg("/bin/sh")
            .arg("-c")
            .arg(if thread_id.is_some() {
                LINUX_RESUME_SCRIPT
            } else {
                LINUX_START_SCRIPT
            })
            .arg("plow-terminal")
            .arg(codex.as_os_str());
        if let Some(thread_id) = thread_id {
            command.arg(thread_id);
        }
        command
            .arg(cwd.as_os_str())
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let Ok(child) = command.spawn() else {
            continue;
        };
        if terminal_launch_succeeded(child) {
            let action = if thread_id.is_some() {
                "Opening the thread"
            } else {
                "Starting Codex"
            };
            return Ok(format!("{action} in {terminal}"));
        }
    }

    Err("No working terminal could be opened. Make sure a supported desktop terminal is installed (for example GNOME Terminal, Konsole, Kitty, or xterm), then try again.".to_string())
}

#[cfg(target_os = "linux")]
fn terminal_launch_succeeded(mut child: Child) -> bool {
    let deadline = Instant::now() + Duration::from_millis(400);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() >= deadline => {
                thread::spawn(move || {
                    let _ = child.wait();
                });
                return true;
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_uuid_thread_ids() {
        let result = open_thread(Path::new("codex"), "$(unsafe)", "/tmp");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_relative_working_directories() {
        let result = open_thread(
            Path::new("codex"),
            "019f5ade-99ad-7ed1-b2f3-159136634cf7",
            "relative/path",
        );
        assert!(result.is_err());
    }

    #[test]
    fn validates_configured_executable_paths() {
        assert!(validate_executable_path("relative/codex").is_err());
        assert!(validate_executable_path(
            std::env::current_exe()
                .expect("test executable")
                .to_str()
                .expect("utf-8 test path")
        )
        .is_ok());
    }

    #[test]
    fn validates_absolute_directory_paths() {
        assert!(validate_directory_path("relative/path", "test folder").is_err());
        assert!(validate_directory_path("/tmp", "test folder").is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn tries_the_next_linux_terminal_after_an_immediate_failure() {
        let result = open_linux_with_candidates(
            Path::new("/bin/true"),
            None,
            Path::new("/tmp"),
            &[("/bin/false", &[]), ("/bin/true", &[])],
        )
        .expect("fallback terminal");
        assert!(result.contains("/bin/true"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn quotes_shell_values() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
