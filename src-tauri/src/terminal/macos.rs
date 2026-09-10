use crate::shell::shell_quote;
use std::{env, fs, io::Write, os::unix::fs::OpenOptionsExt, path::Path, process::Command};
use uuid::Uuid;

// Keep AppleScript source fixed: the shell command is an osascript argument.
// Without an `in` target, `do script` creates a new Terminal window.
const TERMINAL_AUTOMATION: &str = r#"on run argv
    tell application "Terminal"
        do script (item 1 of argv)
        activate
    end tell
end run"#;

pub(super) fn open_script(script: &str, action: &str) -> Result<String, String> {
    let path = env::temp_dir().join(format!("plow-terminal-{}.sh", Uuid::new_v4()));
    open_script_at(
        &path,
        script,
        action,
        &mut Command::new("/usr/bin/osascript"),
    )
}

fn open_script_at(
    path: &Path,
    script: &str,
    action: &str,
    automation: &mut Command,
) -> Result<String, String> {
    // The script is input to the system interpreter, never a Terminal document
    // or a directly executable file. Create it privately without following links.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("Could not prepare terminal handoff: {error}"))?;
    let result = (|| {
        file.write_all(script.as_bytes())
            .map_err(|error| format!("Could not write terminal handoff: {error}"))?;
        drop(file);
        let command = format!("/bin/sh {}", shell_quote(&path.to_string_lossy()));
        let output = automation
            .args(["-e", TERMINAL_AUTOMATION])
            .arg(command)
            .output()
            .map_err(|error| format!("Could not run Terminal automation: {error}"))?;
        if !output.status.success() {
            let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let details = if details.is_empty() {
                format!("osascript exited with {}", output.status)
            } else {
                details
            };
            let hint = if details.contains("-1743") {
                " Allow Plow to control Terminal in System Settings → Privacy & Security → Automation."
            } else {
                ""
            };
            return Err(format!("Could not run Codex in Terminal: {details}{hint}"));
        }
        Ok(action.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    // On success the script removes itself after the Codex/SSH session exits.
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = env::temp_dir().join(format!("plow-terminal-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn runner(&self, script: &str) -> Command {
            let path = self.0.join("osascript");
            fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
            let mut command = Command::new("/bin/sh");
            command.arg(path);
            command
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn runs_private_script_as_interpreter_input() {
        check_interpreter_launch("/bin/bash");
    }

    #[test]
    fn runs_script_from_a_fish_terminal() {
        match Command::new("fish").arg("--version").output() {
            Ok(output) => assert!(output.status.success()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("Skipping Fish integration test: fish is not installed");
                return;
            }
            Err(error) => panic!("Could not run fish: {error}"),
        }
        check_interpreter_launch("fish");
    }

    fn check_interpreter_launch(shell: &str) {
        let directory = TestDirectory::new();
        let path = directory.0.join("farmer's \\ $field.sh");
        let source = directory.0.join("automation.txt");
        let mut runner = directory.runner(&format!(
            "test \"$1\" = '-e' || exit 10\nprintf '%s' \"$2\" > {}\nexec {} -c \"$3\"",
            shell_quote(&source.to_string_lossy()),
            shell_quote(shell)
        ));
        let script = "trap 'rm -f -- \"$0\"' EXIT\ntest -r \"$0\" && test ! -x \"$0\"\n";
        assert_eq!(
            open_script_at(&path, script, "Starting Codex", &mut runner).unwrap(),
            "Starting Codex"
        );
        assert_eq!(fs::read_to_string(source).unwrap(), TERMINAL_AUTOMATION);
        assert!(
            !path.exists(),
            "the script must remove itself after running"
        );
    }

    #[test]
    fn reports_automation_denial_and_cleans_up_script() {
        let directory = TestDirectory::new();
        let path = directory.0.join("session.sh");
        let mut runner = directory.runner(
            "printf 'Not authorized to send Apple events to Terminal. (-1743)' >&2\nexit 1",
        );
        let error = open_script_at(&path, "exit 0", "Starting Codex", &mut runner).unwrap_err();
        assert!(error.contains("-1743"));
        assert!(error.contains("Privacy & Security → Automation"));
        assert!(!path.exists());
    }

    #[test]
    fn cleans_up_script_when_automation_cannot_start() {
        let directory = TestDirectory::new();
        let path = directory.0.join("session.sh");
        let error = open_script_at(
            &path,
            "exit 0",
            "Starting Codex",
            &mut Command::new(directory.0.join("missing-osascript")),
        )
        .unwrap_err();
        assert!(error.contains("Could not run Terminal automation"));
        assert!(!path.exists());
    }

    #[test]
    fn does_not_overwrite_or_remove_an_existing_file() {
        let directory = TestDirectory::new();
        let path = directory.0.join("session.sh");
        fs::write(&path, "existing file").unwrap();
        assert!(open_script_at(
            &path,
            "exit 0",
            "Starting Codex",
            &mut Command::new("/bin/true")
        )
        .is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "existing file");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn terminal_automation_compiles() {
        let directory = TestDirectory::new();
        let output = Command::new("/usr/bin/osacompile")
            .arg("-o")
            .arg(directory.0.join("terminal.scpt"))
            .args(["-e", TERMINAL_AUTOMATION])
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
    }
}
