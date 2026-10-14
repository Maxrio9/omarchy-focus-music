use crate::{controller::Controller, util};
use anyhow::Result;
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Write,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use zbus::{
    blocking::{Connection, Proxy, connection::Builder},
    zvariant::{OwnedObjectPath, OwnedValue, Value as BusValue},
};
const NAME: &str = "org.mpris.MediaPlayer2.focusmusic";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
#[derive(Clone)]
struct Shared {
    status: Arc<Mutex<Value>>,
    binary: std::path::PathBuf,
}
impl Shared {
    fn action(&self, args: &[&str]) {
        if let Ok(child) = Command::new(&self.binary)
            .arg("music")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            util::reap(child);
        }
    }
    fn status(&self) -> Value {
        self.status.lock().unwrap().clone()
    }
    fn phase(&self) -> String {
        let status = self.status();
        let phase = util::text(&status["phase"]);
        if ["Playing", "Loading"].contains(&phase) {
            "Playing".into()
        } else if phase == "Paused" {
            "Paused".into()
        } else {
            "Stopped".into()
        }
    }
}
struct Root;
#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {}
    fn quit(&self) {}
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Focus Music"
    }
    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        "focusmusic"
    }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        Vec::new()
    }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}
struct Player(Shared);
#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn next(&self) {
        if self.can_go_next() {
            self.0.action(&["skip", "keep-pause"]);
        }
    }
    fn previous(&self) {
        if self.can_go_previous() {
            self.0.action(&["previous", "keep-pause"]);
        }
    }
    fn pause(&self) {
        self.0.action(&["pause-set", "1"]);
    }
    fn play_pause(&self) {
        self.0.action(&["toggle"]);
    }
    fn stop(&self) {
        self.0.action(&["stop"]);
    }
    fn play(&self) {
        self.0.action(&["pause-set", "0"]);
    }
    fn seek(&self, _offset: i64) {}
    fn set_position(&self, _track_id: OwnedObjectPath, _position: i64) {}
    fn open_uri(&self, _uri: &str) {}
    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.0.phase()
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        metadata(&self.0.status())
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        util::number(&self.0.status()["volume"], 20.) / 100.
    }
    #[zbus(property)]
    fn set_volume(&self, value: f64) {
        if value.is_finite() {
            self.0
                .action(&["volume-set", &(value * 100.).clamp(0., 100.).to_string()]);
        }
    }
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        0
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.
    }
    #[zbus(property)]
    fn set_rate(&self, _value: f64) {}
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.0.phase() != "Stopped"
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.0.phase() != "Stopped"
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}
fn metadata(status: &Value) -> HashMap<String, OwnedValue> {
    let path = OwnedObjectPath::try_from(format!(
        "{PATH}/track/{}",
        util::digest(util::text(&status["track_id"]).as_bytes())
    ))
    .unwrap();
    let artist = vec![util::text(&status["artist"]).to_string()];
    HashMap::from([
        (
            "mpris:trackid".into(),
            OwnedValue::try_from(BusValue::from(path)).unwrap(),
        ),
        (
            "xesam:title".into(),
            OwnedValue::try_from(BusValue::from(util::text(&status["title"]))).unwrap(),
        ),
        (
            "xesam:artist".into(),
            OwnedValue::try_from(BusValue::from(artist)).unwrap(),
        ),
    ])
}
fn other_playing(connection: &Connection) -> Result<bool> {
    let dbus = Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )?;
    let names: Vec<String> = dbus.call("ListNames", &())?;
    for name in names {
        if !name.starts_with("org.mpris.MediaPlayer2.")
            || name == NAME
            || name.contains("playerctld")
        {
            continue;
        }
        if let Ok(proxy) = Proxy::new(connection, name.as_str(), PATH, PLAYER)
            && proxy
                .get_property::<String>("PlaybackStatus")
                .ok()
                .as_deref()
                == Some("Playing")
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn changed(connection: &Connection, shared: &Shared) -> Result<()> {
    let status = shared.status();
    let values: HashMap<String, OwnedValue> = HashMap::from([
        (
            "PlaybackStatus".into(),
            OwnedValue::try_from(BusValue::from(shared.phase())).unwrap(),
        ),
        (
            "Metadata".into(),
            OwnedValue::try_from(BusValue::from(metadata(&status))).unwrap(),
        ),
        (
            "Volume".into(),
            OwnedValue::from(util::number(&status["volume"], 20.) / 100.),
        ),
        (
            "CanGoNext".into(),
            OwnedValue::from(shared.phase() != "Stopped"),
        ),
        (
            "CanGoPrevious".into(),
            OwnedValue::from(shared.phase() != "Stopped"),
        ),
    ]);
    connection.emit_signal(
        None::<&str>,
        PATH,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
        &(PLAYER, values, Vec::<String>::new()),
    )?;
    Ok(())
}
pub fn run(controller: Controller) -> Result<()> {
    let Ok(_lock) = util::Lock::acquire(&controller.paths.runtime.join("media.lock"), true) else {
        return Ok(());
    };
    let shared = Shared {
        status: Arc::new(Mutex::new(controller.status())),
        binary: std::env::current_exe()?,
    };
    let connection = Builder::session()?
        .method_timeout(Duration::from_millis(500))
        .serve_at(PATH, Root)?
        .serve_at(PATH, Player(shared.clone()))?
        .build()?;
    let controller = Arc::new(Mutex::new(controller));
    let server = crate::rpc::Server::start(controller.clone())?;
    let mut owned = false;
    let mut previous = String::new();
    let mut sent = Instant::now() - Duration::from_secs(3);
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut error = None;
    while !util::stopping() {
        let tick = (|| -> Result<()> {
            let snapshot = {
                let mut controller = controller.lock().unwrap();
                controller.reload()?;
                controller.clone()
            };
            let active = other_playing(&connection)?;
            snapshot.sync_other_media(active)?;
            let status = snapshot.status();
            let encoded = serde_json::to_string(&status)?;
            if active && owned {
                connection.release_name(NAME)?;
                owned = false;
            } else if !active && !owned {
                connection.request_name(NAME)?;
                owned = true;
            }
            *shared.status.lock().unwrap() = status;
            if encoded != previous {
                changed(&connection, &shared)?;
            }
            if encoded != previous || sent.elapsed() >= Duration::from_secs(2) {
                writeln!(output, "{encoded}")?;
                output.flush()?;
                sent = Instant::now();
                previous = encoded;
            }
            Ok(())
        })();
        if let Err(e) = tick {
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe)
            {
                error = Some(e);
                break;
            }
            eprintln!("Focus media: {e}");
        }
        server.wait();
    }
    drop(server);
    // Only restore a pause owned by this bridge; preserve manually paused music.
    let _ = controller.lock().unwrap().sync_other_media(false);
    if owned {
        let _ = connection.release_name(NAME);
    }
    drop(connection);
    if let Some(error) = error {
        Err(error)
    } else {
        Ok(())
    }
}
