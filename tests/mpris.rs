mod common;
use common::Fixture;
use omarchy_focus_music::util;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use zbus::{
    blocking::{Connection, Proxy, connection::Builder},
    zvariant::OwnedValue,
};
struct Video(Arc<AtomicBool>);
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Video {
    #[zbus(property)]
    fn playback_status(&self) -> &str {
        if self.0.load(Ordering::Relaxed) {
            "Playing"
        } else {
            "Paused"
        }
    }
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "Media condition did not settle");
        std::thread::sleep(Duration::from_millis(30));
    }
}
fn names(bus: &Connection) -> Vec<String> {
    Proxy::new(
        bus,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .unwrap()
    .call("ListNames", &())
    .unwrap()
}
fn focus(bus: &Connection) -> Proxy<'_> {
    Proxy::new(
        bus,
        "org.mpris.MediaPlayer2.focusmusic",
        "/org/mpris/MediaPlayer2",
        "org.mpris.MediaPlayer2.Player",
    )
    .unwrap()
}
struct Daemon(Child);
impl Drop for Daemon {
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.0.id() as i32, libc::SIGTERM);
        }
        let end = Instant::now() + Duration::from_secs(4);
        while self.0.try_wait().unwrap().is_none() {
            if Instant::now() > end {
                let _ = self.0.kill();
                let _ = self.0.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
fn daemon(f: &Fixture) -> Daemon {
    let p = &f.controller.paths;
    Daemon(
        Command::new(env!("CARGO_BIN_EXE_focus-music"))
            .arg("media")
            .env("FOCUS_MUSIC_PACKAGE", &p.package)
            .env("FOCUS_MUSIC_CONFIG", p.config_home.join("sources.json"))
            .env("FOCUS_MUSIC_LOCAL_CONFIG", &p.local)
            .env("FOCUS_MUSIC_STATE", &p.state)
            .env("FOCUS_MUSIC_RUNTIME", &p.runtime)
            .env("FOCUS_MUSIC_PREFERENCES", &p.preferences)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}
#[test]
fn media_keys_and_handoffs_in_private_session() {
    if std::env::var_os("FOCUS_PRIVATE_BUS_TEST").is_none() {
        if util::executable("dbus-run-session").is_none() {
            eprintln!("dbus-run-session unavailable; private bus test skipped");
            return;
        }
        let output = Command::new("dbus-run-session")
            .arg("--")
            .arg(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "media_keys_and_handoffs_in_private_session",
                "--nocapture",
            ])
            .env("FOCUS_PRIVATE_BUS_TEST", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let f = Fixture::new();
    let p = &f.controller;
    p.play("piano").unwrap();
    let bus = Connection::session().unwrap();
    let active = Arc::new(AtomicBool::new(false));
    let video = Builder::session()
        .unwrap()
        .name("org.mpris.MediaPlayer2.testvideo")
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", Video(active.clone()))
        .unwrap()
        .build()
        .unwrap();
    let mut service = daemon(&f);
    let stream = service.0.stdout.take().unwrap();
    let frames = Arc::new(Mutex::new(Vec::<Value>::new()));
    let collected = frames.clone();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if let Ok(value) = serde_json::from_str(&line) {
                collected.lock().unwrap().push(value);
            }
        }
    });
    wait(|| names(&bus).contains(&"org.mpris.MediaPlayer2.focusmusic".to_string()));
    assert_eq!(
        focus(&bus)
            .get_property::<String>("PlaybackStatus")
            .unwrap(),
        "Playing"
    );
    let metadata = focus(&bus)
        .get_property::<std::collections::HashMap<String, OwnedValue>>("Metadata")
        .unwrap();
    let title =
        String::try_from(metadata.get("xesam:title").unwrap().try_clone().unwrap()).unwrap();
    assert_eq!(title, p.status()["title"].as_str().unwrap());
    focus(&bus).call::<_, _, ()>("PlayPause", &()).unwrap();
    wait(|| p.status()["phase"] == "Paused");
    focus(&bus).call::<_, _, ()>("Next", &()).unwrap();
    wait(|| p.prop("playlist-pos").ok() == Some(json!(1)));
    assert_eq!(p.status()["phase"], "Paused");
    focus(&bus).call::<_, _, ()>("Previous", &()).unwrap();
    wait(|| p.prop("playlist-pos").ok() == Some(json!(0)));
    focus(&bus).call::<_, _, ()>("Play", &()).unwrap();
    wait(|| p.status()["phase"] == "Playing");
    active.store(true, Ordering::Relaxed);
    wait(|| {
        p.status()["phase"] == "Paused"
            && !names(&bus).contains(&"org.mpris.MediaPlayer2.focusmusic".to_string())
    });
    assert_eq!(p.state()["auto_paused"], true);
    // A duplicate widget must not steal ownership or undo the first bridge's pause.
    let duplicate = daemon(&f);
    drop(duplicate);
    assert_eq!(p.status()["phase"], "Paused");
    active.store(false, Ordering::Relaxed);
    wait(|| {
        p.status()["phase"] == "Playing"
            && names(&bus).contains(&"org.mpris.MediaPlayer2.focusmusic".to_string())
    });
    p.action("pause-set", &["1".into()]).unwrap();
    active.store(true, Ordering::Relaxed);
    wait(|| !names(&bus).contains(&"org.mpris.MediaPlayer2.focusmusic".to_string()));
    active.store(false, Ordering::Relaxed);
    wait(|| names(&bus).contains(&"org.mpris.MediaPlayer2.focusmusic".to_string()));
    assert_eq!(p.status()["phase"], "Paused");
    p.configure(&json!({"otherMedia":"duck-gentle"})).unwrap();
    p.action("pause-set", &["0".into()]).unwrap();
    active.store(true, Ordering::Relaxed);
    wait(|| p.prop("volume").ok().and_then(|v| v.as_f64()) == Some(15.));
    assert_eq!(p.status()["phase"], "Playing");
    assert!(!names(&bus).contains(&"org.mpris.MediaPlayer2.focusmusic".to_string()));
    p.action("volume-set", &["40".into()]).unwrap();
    assert_eq!(p.prop("volume").unwrap().as_f64(), Some(30.));
    active.store(false, Ordering::Relaxed);
    wait(|| {
        p.prop("volume").ok().and_then(|v| v.as_f64()) == Some(40.)
            && names(&bus).contains(&"org.mpris.MediaPlayer2.focusmusic".to_string())
    });
    p.configure(&json!({"otherMedia":"none"})).unwrap();
    active.store(true, Ordering::Relaxed);
    wait(|| !names(&bus).contains(&"org.mpris.MediaPlayer2.focusmusic".to_string()));
    assert_eq!(p.prop("volume").unwrap().as_f64(), Some(40.));
    active.store(false, Ordering::Relaxed);
    wait(|| names(&bus).contains(&"org.mpris.MediaPlayer2.focusmusic".to_string()));
    p.configure(&json!({"otherMedia":"pause-resume"})).unwrap();
    active.store(true, Ordering::Relaxed);
    wait(|| p.state()["auto_paused"] == true);
    drop(service);
    reader.join().unwrap();
    assert_eq!(p.status()["phase"], "Playing");
    assert!(frames.lock().unwrap().len() > 3);
    assert!(
        frames
            .lock()
            .unwrap()
            .iter()
            .all(|v| v["preferences"].is_object() && v["generation"].is_string())
    );
    drop(video);
    println!(
        "Native MPRIS: methods, pause ownership, duck restoration, no-action routing, duplicate-instance safety, and cleanup passed"
    );
}
