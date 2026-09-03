use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use uuid::Uuid;

pub fn find_in_path(name: &str) -> Option<PathBuf> {
    if name.contains(std::path::MAIN_SEPARATOR) {
        let path = PathBuf::from(name);
        return path.is_file().then_some(path);
    }

    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

pub fn open_thread(codex: &Path, thread_id: &str, cwd: &str) -> Result<String, String> {
    let id = Uuid::parse_str(thread_id)
        .map_err(|_| "Codex returned an invalid thread id".to_string())?;
    let working_directory = Path::new(cwd);
    if !working_directory.is_absolute() || !working_directory.is_dir() {
        return Err("The thread's working folder is no longer available".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        return open_macos(codex, &id.to_string(), cwd);
    }

    #[cfg(target_os = "linux")]
    {
        return open_linux(codex, &id.to_string(), cwd);
    }

    #[allow(unreachable_code)]
    Err("Terminal handoff is only supported on macOS and Linux".to_string())
}

#[cfg(target_os = "macos")]
fn open_macos(codex: &Path, thread_id: &str, cwd: &str) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;

    let script_path = env::temp_dir().join(format!("plow-resume-{thread_id}.command"));
    let script = format!(
        "#!/bin/sh\ntrap 'rm -f -- \"$0\"' EXIT\ncd -- {} || exit 1\n{} resume {} --remote unix:// --cd {}\nstatus=$?\nif [ \"$status\" -ne 0 ]; then\n  printf '\\nPlow could not resume this Codex thread (exit %s).\\n' \"$status\"\n  printf 'Press Return to close this window. '\n  read -r _\nfi\nexit \"$status\"\n",
        shell_quote(cwd),
        shell_quote(&codex.to_string_lossy()),
        shell_quote(thread_id),
        shell_quote(cwd),
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
    Ok("Opening the thread in Terminal".to_string())
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
fn open_linux(codex: &Path, thread_id: &str, cwd: &str) -> Result<String, String> {
    let candidates: &[(&str, &[&str])] = &[
        ("xdg-terminal-exec", &["--"]),
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("kitty", &["--"]),
        ("wezterm", &["start", "--"]),
        ("foot", &[]),
        ("alacritty", &["-e"]),
        ("xterm", &["-e"]),
    ];

    for (terminal, prefix) in candidates {
        let Some(path) = find_in_path(terminal) else {
            continue;
        };
        let mut command = Command::new(path);
        command
            .args(*prefix)
            .arg("/bin/sh")
            .arg("-c")
            .arg(LINUX_RESUME_SCRIPT)
            .arg("plow-resume")
            .arg(codex.as_os_str())
            .arg(thread_id)
            .arg(cwd)
            .current_dir(cwd);
        if command.spawn().is_ok() {
            return Ok(format!("Opening the thread in {terminal}"));
        }
    }

    Err("No supported terminal was found. Copy the resume command instead.".to_string())
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

    #[cfg(target_os = "macos")]
    #[test]
    fn quotes_shell_values() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
