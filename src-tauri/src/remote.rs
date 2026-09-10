use crate::{terminal, SshHostSettings};
use std::{
    collections::VecDeque,
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, ChildStdin, Command, Output, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

const SSH_OPTIONS: &[&str] = &[
    "-T",
    "-o",
    "BatchMode=yes",
    "-o",
    "ConnectTimeout=10",
    "-o",
    "ServerAliveInterval=15",
    "-o",
    "ServerAliveCountMax=3",
    "-o",
    "ClearAllForwardings=yes",
];

pub fn host_id(alias: &str) -> String {
    format!("ssh:{alias}")
}

pub fn validate_host(host: &SshHostSettings) -> Result<(), String> {
    validate_alias(&host.alias)?;
    validate_remote_command(&host.codex_path)?;
    if !host.development_home.is_empty() {
        validate_remote_directory(&host.development_home, "the remote development home")?;
    }
    Ok(())
}

pub fn validate_alias(alias: &str) -> Result<(), String> {
    if alias.is_empty() {
        return Err("Enter an SSH host alias".to_string());
    }
    if alias.starts_with('-')
        || !alias
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(
            "SSH host aliases may contain letters, numbers, dots, underscores, and hyphens"
                .to_string(),
        );
    }
    Ok(())
}

pub fn validate_remote_command(value: &str) -> Result<(), String> {
    let command = remote_codex_command(value);
    if command.contains(['\0', '\n', '\r']) {
        return Err("The remote Codex command contains an unsupported character".to_string());
    }
    if command.contains('/') && !command.starts_with('/') {
        return Err("Use either a command name or an absolute path for remote Codex".to_string());
    }
    if !command.contains('/')
        && !command
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
    {
        return Err("The remote Codex command name contains an unsupported character".to_string());
    }
    Ok(())
}

pub fn validate_remote_directory(value: &str, label: &str) -> Result<(), String> {
    if value.contains(['\0', '\n', '\r']) || !value.starts_with('/') {
        return Err(format!("Use an absolute Unix path for {label}"));
    }
    normalize_remote_path(value).map(|_| ())
}

pub fn validate_project_selection(home: &str, selected: &str) -> Result<(), String> {
    let home = normalize_remote_path(home)?;
    let selected = normalize_remote_path(selected)?;
    if selected == home {
        return Ok(());
    }
    if selected.parent() != Some(home.as_path()) {
        return Err(
            "Choose the configured remote development home or a project directly inside it"
                .to_string(),
        );
    }
    Ok(())
}

fn normalize_remote_path(value: &str) -> Result<PathBuf, String> {
    validate_remote_directory_shape(value)?;
    let mut normalized = PathBuf::from("/");
    for component in Path::new(value).components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err("The remote path escapes its root".to_string());
                }
            }
            Component::Prefix(_) => return Err("Use an absolute Unix path".to_string()),
        }
    }
    Ok(normalized)
}

fn validate_remote_directory_shape(value: &str) -> Result<(), String> {
    if value.contains(['\0', '\n', '\r']) || !value.starts_with('/') {
        return Err("Use an absolute Unix path".to_string());
    }
    Ok(())
}

pub fn remote_codex_command(value: &str) -> &str {
    if value.is_empty() {
        "codex"
    } else {
        value
    }
}

pub fn run_codex(host: &SshHostSettings, args: &[&str]) -> Result<Output, String> {
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push(remote_codex_command(&host.codex_path).to_string());
    argv.extend(args.iter().map(|value| (*value).to_string()));
    run_argv(&host.alias, &argv)
}

pub fn run_argv(alias: &str, argv: &[String]) -> Result<Output, String> {
    let ssh = ssh_executable()?;
    Command::new(ssh)
        .args(SSH_OPTIONS)
        .arg(alias)
        .arg(login_shell_command(argv))
        .output()
        .map_err(|error| format!("Could not run SSH for {alias}: {error}"))
}

pub fn require_success(output: Output, action: &str, alias: &str) -> Result<Vec<u8>, String> {
    if output.status.success() {
        return Ok(output.stdout);
    }
    let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let details = if details.is_empty() {
        format!("SSH exited with {}", output.status)
    } else {
        details
    };
    Err(format!("Could not {action} on {alias}: {details}"))
}

pub fn spawn_codex_proxy(host: &SshHostSettings) -> Result<SshProxyStream, String> {
    let ssh = ssh_executable()?;
    let argv = vec![
        remote_codex_command(&host.codex_path).to_string(),
        "app-server".to_string(),
        "proxy".to_string(),
    ];
    let child = Command::new(ssh)
        .args(SSH_OPTIONS)
        .arg(&host.alias)
        .arg(login_shell_command(&argv))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not start the Codex SSH proxy: {error}"))?;
    SshProxyStream::new(child)
}

pub fn terminal_arguments(
    host: &SshHostSettings,
    codex_args: &[String],
) -> Result<Vec<String>, String> {
    let ssh = ssh_executable()?;
    let mut argv = vec![ssh.to_string_lossy().into_owned(), "-t".to_string()];
    argv.extend(
        SSH_OPTIONS
            .iter()
            .filter(|value| **value != "-T")
            .map(|value| (*value).to_string()),
    );
    argv.push(host.alias.clone());
    let mut remote = Vec::with_capacity(codex_args.len() + 1);
    remote.push(remote_codex_command(&host.codex_path).to_string());
    remote.extend_from_slice(codex_args);
    argv.push(login_shell_command(&remote));
    Ok(argv)
}

fn ssh_executable() -> Result<PathBuf, String> {
    terminal::find_in_path("ssh")
        .ok_or_else(|| "OpenSSH was not found. Install the ssh command and try again.".to_string())
}

fn login_shell_command(argv: &[String]) -> String {
    let command = argv
        .iter()
        .map(|value| shell_quote(value))
        .collect::<Vec<_>>()
        .join(" ");
    // SSH first parses this with the user's shell, which may be Fish. Keep
    // POSIX parameter expansion inside sh, then load the user's login shell
    // so Codex still gets its configured PATH. Pass the command as data.
    format!(
        "exec /bin/sh -c 'exec \"${{SHELL:-/bin/sh}}\" -lc \"$1\"' plow-ssh {}",
        shell_quote(&format!("exec {command}"))
    )
}

fn shell_quote(value: &str) -> String {
    let mut quoted = String::from("'");
    for ch in value.chars() {
        match ch {
            '\'' => quoted.push_str("'\\''"),
            // Fish interprets backslashes even inside single quotes. Escape
            // them outside quotes so both Fish and POSIX shells preserve them.
            '\\' => quoted.push_str("'\\\\'"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('\'');
    quoted
}

pub fn discover_aliases() -> Vec<String> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    discover_aliases_from(&PathBuf::from(home).join(".ssh/config"))
}

fn discover_aliases_from(path: &Path) -> Vec<String> {
    let Ok(config) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut aliases = Vec::new();
    for raw_line in config.lines() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        let mut fields = line.split_whitespace();
        if !fields
            .next()
            .is_some_and(|field| field.eq_ignore_ascii_case("host"))
        {
            continue;
        }
        for alias in fields {
            let alias = alias.trim_matches(['\'', '"']);
            if !alias.contains(['*', '?', '!']) && validate_alias(alias).is_ok() {
                aliases.push(alias.to_string());
            }
        }
    }
    aliases.sort_by_key(|alias| alias.to_lowercase());
    aliases.dedup();
    aliases
}

pub struct SshProxyStream {
    child: Child,
    writer: ChildStdin,
    receiver: mpsc::Receiver<io::Result<Vec<u8>>>,
    pending: VecDeque<u8>,
    read_timeout: Duration,
    stderr: Arc<Mutex<String>>,
}

impl SshProxyStream {
    fn new(mut child: Child) -> Result<Self, String> {
        let writer = child
            .stdin
            .take()
            .ok_or_else(|| "The SSH proxy did not provide stdin".to_string())?;
        let mut reader = child
            .stdout
            .take()
            .ok_or_else(|| "The SSH proxy did not provide stdout".to_string())?;
        let mut error_reader = child
            .stderr
            .take()
            .ok_or_else(|| "The SSH proxy did not provide stderr".to_string())?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || loop {
            let mut chunk = vec![0_u8; 8192];
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(size) => {
                    chunk.truncate(size);
                    if sender.send(Ok(chunk)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        });
        let stderr = Arc::new(Mutex::new(String::new()));
        let stderr_writer = stderr.clone();
        thread::spawn(move || {
            let mut buffer = [0_u8; 2048];
            loop {
                match error_reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(size) => {
                        let text = String::from_utf8_lossy(&buffer[..size]);
                        let mut log = stderr_writer.lock().expect("ssh stderr lock");
                        if log.len() < 16_384 {
                            log.push_str(&text);
                        }
                    }
                }
            }
        });
        Ok(Self {
            child,
            writer,
            receiver,
            pending: VecDeque::new(),
            read_timeout: Duration::from_secs(12),
            stderr,
        })
    }

    pub fn set_read_timeout(&mut self, timeout: Duration) {
        self.read_timeout = timeout;
    }

    pub fn error_details(&self) -> String {
        self.stderr
            .lock()
            .expect("ssh stderr lock")
            .trim()
            .to_string()
    }
}

impl Read for SshProxyStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.pending.is_empty() {
            match self.receiver.recv_timeout(self.read_timeout) {
                Ok(Ok(chunk)) => self.pending.extend(chunk),
                Ok(Err(error)) => return Err(error),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "SSH read timed out",
                    ));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let details = self.error_details();
                    if details.is_empty() {
                        return Ok(0);
                    }
                    return Err(io::Error::new(io::ErrorKind::ConnectionAborted, details));
                }
            }
        }
        let size = buffer.len().min(self.pending.len());
        for target in &mut buffer[..size] {
            *target = self.pending.pop_front().expect("pending SSH byte");
        }
        Ok(size)
    }
}

impl Write for SshProxyStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.writer.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl Drop for SshProxyStream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_aliases_that_could_be_ssh_options() {
        assert!(validate_alias("devbox").is_ok());
        assert!(validate_alias("build.example_com").is_ok());
        assert!(validate_alias("-oProxyCommand=bad").is_err());
        assert!(validate_alias("user@host").is_err());
    }

    #[test]
    fn discovers_only_concrete_ssh_hosts() {
        let root = std::env::temp_dir().join(format!("plow-ssh-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config");
        fs::write(
            &config,
            "Host *\n  ServerAliveInterval 30\nHost devbox build-box\nHost *.internal\nHost -unsafe\n",
        )
        .unwrap();
        assert_eq!(discover_aliases_from(&config), vec!["build-box", "devbox"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accepts_remote_home_and_direct_project_children() {
        assert!(validate_project_selection("/srv/dev", "/srv/dev").is_ok());
        assert!(validate_project_selection("/srv/dev", "/srv/dev/plow").is_ok());
        assert!(validate_project_selection("/srv/dev", "/srv/dev/nested/plow").is_err());
        assert!(validate_project_selection("/srv/dev", "/srv/dev/../secret").is_err());
    }
}

#[cfg(test)]
mod shell_tests {
    use super::login_shell_command;
    use std::{io::Write, process::Command, process::Stdio};

    fn check_login_shell(shell: &str) {
        let values = [
            "app-server",
            "daemon",
            "start",
            "/opt/Codex tools/codex",
            "/srv/farmer's field",
            "",
            "\\",
            "\\\\",
            "\\'",
            "trailing\\",
            "$(printf injected)",
            "`printf injected`",
            "; printf injected",
            "\"$SHELL\"",
            "* ? [abc] {a,b}",
            "line1\nline2",
        ];
        let mut argv = vec!["/usr/bin/printf".to_string(), "%s\\0".to_string()];
        argv.extend(values.iter().map(|value| value.to_string()));
        let output = Command::new(shell)
            .args(["-c", &login_shell_command(&argv)])
            .env("SHELL", shell)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        let expected: Vec<u8> = values
            .iter()
            .flat_map(|value| value.bytes().chain(std::iter::once(0)))
            .collect();
        assert_eq!(output.stdout, expected);

        // The proxy needs unchanged stdin/stdout and the remote exit status.
        let argv = [
            "/bin/sh".to_string(),
            "-c".to_string(),
            "cat; printf 'remote error' >&2; exit 23".to_string(),
        ];
        let mut child = Command::new(shell)
            .args(["-c", &login_shell_command(&argv)])
            .env("SHELL", shell)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = b"proxy\0data\n";
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(23));
        assert_eq!(output.stdout, input);
        assert_eq!(output.stderr, b"remote error");
    }

    #[test]
    fn remote_commands_work_in_bash() {
        check_login_shell("/bin/bash");
    }

    #[test]
    fn remote_commands_work_in_fish() {
        match Command::new("fish").arg("--version").output() {
            Ok(output) => assert!(output.status.success()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("Skipping Fish integration test: fish is not installed");
                return;
            }
            Err(error) => panic!("Could not run fish: {error}"),
        }
        check_login_shell("fish");
    }

    #[test]
    fn remote_commands_fall_back_to_sh_when_shell_is_unset_or_empty() {
        let argv = ["/usr/bin/printf".to_string(), "fallback".to_string()];
        for shell in [None, Some("")] {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", &login_shell_command(&argv)]);
            if let Some(shell) = shell {
                command.env("SHELL", shell);
            } else {
                command.env_remove("SHELL");
            }
            let output = command.output().unwrap();
            assert!(output.status.success(), "{:?}", output);
            assert_eq!(output.stdout, b"fallback");
        }
    }
}
