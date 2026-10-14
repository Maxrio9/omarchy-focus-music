use crate::{
    controller::{Controller, Paths},
    util,
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
const SUPPORTED: &[&str] = &[
    "status",
    "genres",
    "play",
    "pause-set",
    "toggle",
    "skip",
    "previous",
    "stop",
    "volume",
    "volume-set",
    "media-sync",
    "restart-player",
    "is-playing",
];
fn context(paths: &Paths) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let files = [
        &paths.package,
        &paths.source(),
        &paths.local,
        &paths.preferences,
        &paths.state,
        &paths.cache,
        &paths.shell,
    ]
    .iter()
    .map(|path| {
        if path.is_absolute() {
            (*path).clone()
        } else {
            cwd.join(path)
        }
    })
    .collect::<Vec<_>>();
    util::digest(serde_json::to_vec(&files).unwrap())
}
pub fn request(paths: &Paths, action: &str, args: &[String]) -> Result<Option<Value>> {
    if !SUPPORTED.contains(&action) {
        return Ok(None);
    }
    let socket = paths.runtime.join("controller.sock");
    let mut stream = match UnixStream::connect(socket) {
        Ok(s) => s,
        Err(e)
            if [
                std::io::ErrorKind::NotFound,
                std::io::ErrorKind::ConnectionRefused,
            ]
            .contains(&e.kind()) =>
        {
            return Ok(None);
        }
        Err(e) => return Err(e.into()),
    };
    stream.set_read_timeout(Some(Duration::from_secs(65)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    writeln!(
        stream,
        "{}",
        json!({"context":context(paths),"action":action,"args":args})
    )?;
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line)? == 0 {
        bail!("Backend did not acknowledge the command; it was not retried");
    }
    let response: Value = serde_json::from_str(&line)?;
    if response["fallback"] == true {
        return Ok(None);
    }
    if response["ok"] != true {
        bail!("{}", util::text(&response["error"]));
    }
    Ok(Some(response["value"].clone()))
}
pub struct Server {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    wake: Arc<(Mutex<bool>, Condvar)>,
}
impl Server {
    pub fn start(controller: Arc<Mutex<Controller>>) -> Result<Self> {
        let paths = controller.lock().unwrap().paths.clone();
        let path = paths.runtime.join("controller.sock");
        if path.exists() {
            if !fs::symlink_metadata(&path)?.file_type().is_socket() {
                bail!("Controller endpoint is not a socket");
            }
            fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let wake = Arc::new((Mutex::new(false), Condvar::new()));
        let notify = wake.clone();
        let handle = thread::spawn(move || {
            let mut workers = Vec::<thread::JoinHandle<()>>::new();
            while !stopped.load(Ordering::Relaxed) && !util::stopping() {
                workers.retain(|worker| !worker.is_finished());
                let mut fd = libc::pollfd {
                    fd: listener.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                let ready = unsafe { libc::poll(&mut fd, 1, 200) };
                if ready <= 0 {
                    continue;
                }
                let Ok((stream, _)) = listener.accept() else {
                    continue;
                };
                let shared = controller.clone();
                let wake = notify.clone();
                workers.push(thread::spawn(move || {
                    let _ = serve(stream, shared, wake);
                }));
            }
            for worker in workers {
                let _ = worker.join();
            }
        });
        Ok(Self {
            path,
            stop,
            thread: Some(handle),
            wake,
        })
    }
    pub fn wait(&self) {
        let (lock, wake) = &*self.wake;
        let pending = lock.lock().unwrap();
        let (mut pending, _) = wake
            .wait_timeout_while(pending, Duration::from_millis(500), |pending| {
                !*pending && !util::stopping()
            })
            .unwrap();
        *pending = false;
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = UnixStream::connect(&self.path);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        let _ = fs::remove_file(&self.path);
    }
}
fn serve(
    mut stream: UnixStream,
    controller: Arc<Mutex<Controller>>,
    wake: Arc<(Mutex<bool>, Condvar)>,
) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut line = String::new();
    let mut reader = BufReader::new(stream.try_clone()?).take(65537);
    let length = reader.read_line(&mut line)?;
    if length == 0 {
        return Ok(());
    }
    if length > 65536 {
        bail!("Command exceeds size limit");
    }
    let value: Value = serde_json::from_str(&line)?;
    let action = util::text(&value["action"]);
    let args = value["args"]
        .as_array()
        .context("Missing command arguments")?
        .iter()
        .map(|v| {
            v.as_str()
                .context("Invalid command argument")
                .map(String::from)
        })
        .collect::<Result<Vec<_>>>()?;
    let snapshot = {
        let mut ctrl = controller.lock().unwrap();
        if value["context"] != context(&ctrl.paths) || !SUPPORTED.contains(&action) {
            writeln!(stream, "{}", json!({"fallback":true}))?;
            return Ok(());
        }
        ctrl.reload()?;
        ctrl.clone()
    };
    let result = match action {
        "status" => Ok(Some(snapshot.status())),
        "genres" => Ok(Some(snapshot.catalog.list())),
        "restart-player" => snapshot.restart_player().map(Some),
        "is-playing" => {
            let status = snapshot.status();
            let matching = status["genre"].as_str() == args.first().map(String::as_str)
                && ["Playing", "Loading", "Paused"].contains(&util::text(&status["phase"]));
            Ok(Some(json!(matching)))
        }
        _ => snapshot.action(action, &args),
    };
    let response = match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error.to_string()}),
    };
    writeln!(stream, "{response}")?;
    if !["status", "genres", "is-playing"].contains(&action) {
        let (lock, signal) = &*wake;
        *lock.lock().unwrap() = true;
        signal.notify_one();
    }
    Ok(())
}
