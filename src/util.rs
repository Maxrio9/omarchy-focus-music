use anyhow::{Context, Result, bail};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{OpenOptionsExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub static STOP: AtomicBool = AtomicBool::new(false);
pub fn stopping() -> bool {
    STOP.load(Ordering::Relaxed)
}
pub fn signal_setup() -> Result<()> {
    ctrlc::set_handler(|| {
        STOP.store(true, Ordering::Relaxed);
    })?;
    Ok(())
}
pub fn nonempty(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}
pub fn home() -> PathBuf {
    nonempty("HOME").unwrap_or_else(|| PathBuf::from("/"))
}
pub fn xdg(name: &str, fallback: &str) -> PathBuf {
    nonempty(name).unwrap_or_else(|| home().join(fallback))
}
pub fn runtime() -> PathBuf {
    nonempty("FOCUS_MUSIC_RUNTIME").unwrap_or_else(|| {
        nonempty("XDG_RUNTIME_DIR")
            .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })))
            .join("focus-music")
    })
}
pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub fn atomic_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.sync_all()?;
    fs::rename(temporary, path)?;
    Ok(())
}
pub fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
pub fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
pub fn seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn generation() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}
pub fn number(value: &Value, default: f64) -> f64 {
    value.as_f64().filter(|n| n.is_finite()).unwrap_or(default)
}
pub fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
pub fn executable(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path).map(|dir| dir.join(name)).find(|p| {
            p.is_file() && fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        })
    })
}
pub struct Lock(File);
impl Lock {
    pub fn acquire(path: &Path, nonblocking: bool) -> Result<Self> {
        if let Some(parent) = path.parent() {
            private_dir(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        let flags = libc::LOCK_EX | if nonblocking { libc::LOCK_NB } else { 0 };
        if unsafe { libc::flock(file.as_raw_fd(), flags) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
pub fn detach(command: &mut Command) -> Result<std::process::Child> {
    command.stdin(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    Ok(command.spawn()?)
}
// Detached players outlive commands, but a persistent backend must still reap them.
pub fn reap(mut child: std::process::Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}
pub struct Captured {
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
pub fn capture(
    command: &mut Command,
    timeout: Duration,
    cancel: Option<&dyn Fn() -> bool>,
) -> Result<Captured> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = detach(command)?;
    let pid = child.id() as i32;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(64 * 1024 * 1024).read_to_end(&mut bytes);
        (result, bytes)
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.take(64 * 1024 * 1024).read_to_end(&mut bytes);
        (result, bytes)
    });
    let start = Instant::now();
    let result = loop {
        if stopping() || start.elapsed() > timeout || cancel.is_some_and(|f| f()) {
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            break Err(anyhow::anyhow!("Process superseded or timed out"));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
                let _ = child.wait();
                break Err(error.into());
            }
        }
    };
    let (out_result, stdout) = out
        .join()
        .map_err(|_| anyhow::anyhow!("Output reader failed"))?;
    let (err_result, stderr) = err
        .join()
        .map_err(|_| anyhow::anyhow!("Error reader failed"))?;
    let status = result?;
    out_result?;
    err_result?;
    Ok(Captured {
        success: status.success(),
        stdout,
        stderr,
    })
}
pub fn run(command: &mut Command) -> Result<()> {
    let result = capture(command, Duration::from_secs(60), None)?;
    if !result.success {
        bail!(
            "Command failed: {}",
            command.get_program().to_string_lossy()
        );
    }
    Ok(())
}
pub fn package_root() -> Result<PathBuf> {
    if let Some(path) = nonempty("FOCUS_MUSIC_PACKAGE") {
        return Ok(path);
    }
    let exe = env::current_exe()?;
    for parent in exe.ancestors().skip(1) {
        if parent.join("manifest.json").is_file() && parent.join("data/sources.json").is_file() {
            return Ok(parent.to_path_buf());
        }
    }
    bail!("Cannot locate the Focus Music package; set FOCUS_MUSIC_PACKAGE")
}
pub fn expand(path: &str) -> PathBuf {
    if path == "~" {
        home()
    } else if let Some(rest) = path.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(path)
    }
}
pub fn checked_relative(root: &Path, value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        bail!("Invalid plugin path");
    }
    Ok(root.join(path))
}
pub fn backup_name(prefix: &str) -> String {
    format!("{prefix}-{}-{}", generation(), std::process::id())
}
pub fn notify(message: &str) {
    if let Some(program) = executable("notify-send") {
        let _ = Command::new(program)
            .args(["Focus music", message])
            .status();
    }
}
pub fn finite_arg(value: Option<&String>, name: &str) -> Result<f64> {
    let value = value.context(format!("Missing {name}"))?.parse::<f64>()?;
    if !value.is_finite() {
        bail!("Invalid {name}");
    }
    Ok(value)
}
// Retain the original Python cache and local-index identity representation.
pub fn python_json(value: &Value, sort: bool, spaces: bool) -> String {
    fn quote(text: &str) -> String {
        let escaped = serde_json::to_string(text).unwrap();
        let mut out = String::new();
        for ch in escaped.chars() {
            if ch.is_ascii() {
                out.push(ch);
            } else {
                let mut buffer = [0u16; 2];
                for word in ch.encode_utf16(&mut buffer) {
                    out.push_str(&format!("\\u{word:04x}"));
                }
            }
        }
        out
    }
    match value {
        Value::String(s) => quote(s),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(|v| python_json(v, sort, spaces))
                .collect::<Vec<_>>()
                .join(if spaces { ", " } else { "," })
        ),
        Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            if sort {
                entries.sort_by(|a, b| a.0.cmp(b.0));
            }
            format!(
                "{{{}}}",
                entries
                    .iter()
                    .map(|(key, value)| format!(
                        "{}{}{}",
                        quote(key),
                        if spaces { ": " } else { ":" },
                        python_json(value, sort, spaces)
                    ))
                    .collect::<Vec<_>>()
                    .join(if spaces { ", " } else { "," })
            )
        }
        _ => value.to_string(),
    }
}
