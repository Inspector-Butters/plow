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

pub fn open_thread(codex: &Path, thread_id: &str) -> Result<String, String> {
    let id = Uuid::parse_str(thread_id)
        .map_err(|_| "Codex returned an invalid thread id".to_string())?;

    #[cfg(target_os = "macos")]
    {
        return open_macos(codex, &id.to_string());
    }

    #[cfg(target_os = "linux")]
    {
        return open_linux(codex, &id.to_string());
    }

    #[allow(unreachable_code)]
    Err("Terminal handoff is only supported on macOS and Linux".to_string())
}

#[cfg(target_os = "macos")]
fn open_macos(codex: &Path, thread_id: &str) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;

    let script_path = env::temp_dir().join(format!("plow-resume-{thread_id}.command"));
    let script = format!(
        "#!/bin/sh\nexec {} --remote unix:// resume {}\n",
        shell_quote(&codex.to_string_lossy()),
        shell_quote(thread_id)
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
fn open_linux(codex: &Path, thread_id: &str) -> Result<String, String> {
    let codex_arg = codex.as_os_str();
    let candidates: &[(&str, &[&str])] = &[
        ("xdg-terminal-exec", &[]),
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("kitty", &[]),
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
            .arg(codex_arg)
            .args(["--remote", "unix://", "resume", thread_id]);
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
        let result = open_thread(Path::new("codex"), "$(unsafe)");
        assert!(result.is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn quotes_shell_values() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }
}
