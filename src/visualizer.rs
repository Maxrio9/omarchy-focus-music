use crate::{
    controller::Paths,
    ipc, preferences,
    spectrum::{RATE, SIZE, Spectrum, bar_count},
    util,
};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
fn id(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}
pub fn player_node(objects: &[Value], pid: u32) -> Option<u64> {
    let clients: HashSet<_> = objects
        .iter()
        .filter(|v| id(&v["info"]["props"]["application.process.id"]) == Some(pid as u64))
        .filter_map(|v| id(&v["id"]))
        .collect();
    objects.iter().find_map(|v| {
        let props = &v["info"]["props"];
        if util::text(&v["type"]).ends_with(":Node")
            && props["media.class"] == "Stream/Output/Audio"
            && (id(&props["application.process.id"]) == Some(pid as u64)
                || id(&props["client.id"]).is_some_and(|id| clients.contains(&id)))
        {
            id(&props["object.serial"]).or_else(|| id(&v["id"]))
        } else {
            None
        }
    })
}
fn target(pid: u32) -> Result<Option<u64>> {
    let output = util::capture(&mut Command::new("pw-dump"), Duration::from_secs(5), None)?;
    if !output.success {
        bail!("Player output unavailable");
    }
    let objects: Vec<Value> = serde_json::from_slice(&output.stdout)?;
    Ok(player_node(&objects, pid))
}
fn peer_pid(socket: &Path) -> Result<u32> {
    let stream = UnixStream::connect(socket)?;
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if cred.pid <= 0 || cred.uid != unsafe { libc::getuid() } {
        bail!("Invalid player peer");
    }
    Ok(cred.pid as u32)
}
struct Capture(Child);
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn frame(output: &mut impl Write, bands: Vec<f64>, popup: Vec<f64>, state: &str) -> Result<()> {
    serde_json::to_writer(
        &mut *output,
        &json!({"bands":bands,"popupBands":popup,"state":state}),
    )?;
    writeln!(output)?;
    output.flush()?;
    Ok(())
}
fn count(paths: &Paths) -> usize {
    bar_count(
        preferences::read(&paths.preferences, &paths.shell)["visualizerWidth"]
            .as_u64()
            .unwrap_or(100) as usize,
    )
}
pub fn run(paths: &Paths) -> Result<()> {
    let result = live(paths);
    if let Err(error) = result
        && !util::stopping()
    {
        eprintln!("Focus visualizer: {error}");
        frame(
            &mut io::stdout(),
            vec![0.; count(paths)],
            vec![0.; 5],
            "unavailable",
        )?;
    }
    Ok(())
}
fn live(paths: &Paths) -> Result<()> {
    let pid = peer_pid(&paths.socket)?;
    let serial = target(pid)?.ok_or_else(|| anyhow::anyhow!("Player output unavailable"))?;
    let child = Command::new("pw-cat")
        .args([
            "--record",
            "--raw",
            "--format",
            "f32",
            "--rate",
            &RATE.to_string(),
            "--channels",
            "1",
            "--latency",
            "50ms",
            "--target",
            &serial.to_string(),
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut capture = Capture(child);
    let mut input = capture.0.stdout.take().unwrap();
    let mut fft = Spectrum::default();
    let mut buffer = Vec::with_capacity(SIZE * 8);
    let mut scratch = [0u8; SIZE * 4];
    let mut samples = [0f64; SIZE];
    let mut bands = count(paths);
    let mut volume = 100.;
    let mut checked = Instant::now() - Duration::from_secs(1);
    let mut node_checked = Instant::now();
    let mut playlist = Value::Null;
    let mut rebind = Instant::now();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    while !util::stopping() {
        if checked.elapsed() >= Duration::from_millis(250) {
            checked = Instant::now();
            volume = util::number(&ipc::property(&paths.socket, "volume")?, 100.);
            bands = count(paths);
            let current = ipc::property(&paths.socket, "playlist-pos")?;
            if current != playlist {
                playlist = current;
                rebind = Instant::now() + Duration::from_secs(2);
            }
            if Instant::now() < rebind {
                node_checked = Instant::now();
                if target(pid)? != Some(serial) {
                    break;
                }
            }
        }
        let mut fd = libc::pollfd {
            fd: input.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut fd, 1, 200) };
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        if ready == 0 {
            buffer.clear();
            frame(&mut output, vec![0.; bands], vec![0.; 5], "waiting")?;
            if node_checked.elapsed() > Duration::from_secs(1) {
                node_checked = Instant::now();
                if target(pid)? != Some(serial) {
                    break;
                }
            }
            continue;
        }
        let n = input.read(&mut scratch)?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&scratch[..n]);
        while buffer.len() >= SIZE * 4 {
            for (i, chunk) in buffer[..SIZE * 4].as_chunks::<4>().0.iter().enumerate() {
                samples[i] = f32::from_le_bytes(*chunk) as f64;
            }
            buffer.drain(..SIZE * 4);
            let mut results = fft.analyze(&samples, volume, &[bands, 5]).into_iter();
            frame(
                &mut output,
                results.next().unwrap(),
                results.next().unwrap(),
                "active",
            )?;
        }
    }
    Ok(())
}
