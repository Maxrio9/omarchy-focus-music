use crate::{
    catalog::{Catalog, Genre, Track},
    ipc::{self, Ipc},
    preferences, util,
};
use anyhow::{Context, Result, bail};
use rand::seq::SliceRandom;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, UNIX_EPOCH},
};
#[derive(Clone)]
pub struct Paths {
    pub package: PathBuf,
    pub config_home: PathBuf,
    pub local: PathBuf,
    pub preferences: PathBuf,
    pub runtime: PathBuf,
    pub socket: PathBuf,
    pub state: PathBuf,
    pub shell: PathBuf,
    pub cache: PathBuf,
}
impl Paths {
    pub fn new(package: PathBuf) -> Result<Self> {
        let config_home = util::xdg("XDG_CONFIG_HOME", ".config").join("focus-music");
        let local = util::nonempty("FOCUS_MUSIC_LOCAL_CONFIG")
            .unwrap_or_else(|| config_home.join("local-sources.json"));
        let preferences = util::nonempty("FOCUS_MUSIC_PREFERENCES")
            .unwrap_or_else(|| config_home.join("preferences.json"));
        let runtime = util::runtime();
        util::private_dir(&runtime)?;
        let socket = runtime.join("mpv.sock");
        let state = util::nonempty("FOCUS_MUSIC_STATE").unwrap_or_else(|| {
            util::xdg("XDG_STATE_HOME", ".local/state").join("focus-music/state.json")
        });
        if let Some(parent) = state.parent() {
            fs::create_dir_all(parent)?;
        }
        let shell = config_home.parent().unwrap().join("omarchy/shell.json");
        let cache = util::nonempty("FOCUS_MUSIC_CACHE")
            .unwrap_or_else(|| util::xdg("XDG_CACHE_HOME", ".cache").join("focus-music/loudness"));
        Ok(Self {
            package,
            config_home,
            local,
            preferences,
            runtime,
            socket,
            state,
            shell,
            cache,
        })
    }
    pub fn source(&self) -> PathBuf {
        util::nonempty("FOCUS_MUSIC_CONFIG").unwrap_or_else(|| {
            let user = self.config_home.join("sources.json");
            if user.is_file() {
                user
            } else {
                self.package.join("data/sources.json")
            }
        })
    }
}
#[derive(Clone)]
pub struct Controller {
    pub paths: Paths,
    pub catalog: std::sync::Arc<Catalog>,
    signature: String,
}
impl Controller {
    pub fn new(paths: Paths) -> Result<Self> {
        let catalog = Catalog::load(&paths.source(), &paths.local)?;
        let signature = Self::signature(&paths);
        Ok(Self {
            paths,
            catalog: std::sync::Arc::new(catalog),
            signature,
        })
    }
    fn signature(paths: &Paths) -> String {
        [paths.source(), paths.local.clone()]
            .iter()
            .map(|path| {
                let meta = fs::metadata(path).ok();
                format!(
                    "{}:{:?}:{:?}",
                    path.display(),
                    meta.as_ref().map(|m| m.len()),
                    meta.and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|t| t.as_nanos())
                )
            })
            .collect::<Vec<_>>()
            .join("|")
    }
    pub fn reload(&mut self) -> Result<()> {
        let next = Self::signature(&self.paths);
        if next != self.signature {
            self.catalog =
                std::sync::Arc::new(Catalog::load(&self.paths.source(), &self.paths.local)?);
            self.signature = next;
        }
        Ok(())
    }
    pub fn preferences(&self) -> Value {
        preferences::read(&self.paths.preferences, &self.paths.shell)
    }
    pub fn state(&self) -> Value {
        for path in [&self.paths.state, &self.paths.runtime.join("state.json")] {
            if let Ok(mut saved) = util::read_json(path)
                && saved.is_object()
            {
                if self.catalog.get(util::text(&saved["genre"])).is_none() {
                    saved["genre"] = json!(self.catalog.first());
                }
                return saved;
            }
        }
        json!({"genre":self.catalog.first(),"volume":20})
    }
    pub fn save(&self, value: &Value) -> Result<()> {
        util::atomic_json(&self.paths.state, value)
    }
    pub fn prop(&self, name: &str) -> Result<Value> {
        ipc::property(&self.paths.socket, name)
    }
    pub fn command(&self, args: Value) -> Result<Value> {
        ipc::command(&self.paths.socket, args)
    }
    pub fn source<'a>(&'a self, saved: &Value) -> &'a Genre {
        self.catalog
            .get(util::text(&saved["genre"]))
            .unwrap_or(&self.catalog.genres[0].1)
    }
    pub fn active_track(
        &self,
        saved: &Value,
        position: f64,
        index: usize,
        path: &str,
    ) -> (usize, Track) {
        let source = self.source(saved);
        if saved["catalog_version"] == source.catalog_version
            && let Some(order) = saved["active_order"].as_array()
            && !order.is_empty()
            && let Some(number) = order[index.min(order.len() - 1)].as_u64()
            && let Some(track) = source.tracks.get(number as usize)
        {
            return (number as usize, track.clone());
        }
        for (number, track) in source.tracks.iter().enumerate() {
            if track.url == path && track.start <= position && position < track.end {
                return (number, track.clone());
            }
        }
        (
            0,
            Track {
                id: "unknown".into(),
                artist: String::new(),
                title: "Choose a genre".into(),
                url: String::new(),
                start: 0.,
                end: 1.,
                duration: 1.,
                fingerprint: None,
                source_identity: None,
            },
        )
    }
    pub fn status(&self) -> Value {
        let saved = self.state();
        let source = self.source(&saved);
        let mut phase = "Stopped";
        let mut effective = util::number(&saved["volume"], 20.);
        let (mut position, mut index, mut path) = (0., 0usize, String::new());
        let mut normalization = json!({"state":"off","gain_db":0});
        if let Ok(mut client) = Ipc::connect(&self.paths.socket) {
            if let (Ok(idle), Ok(paused), Ok(volume), Ok(track_path)) = (
                client.property("idle-active"),
                client.property("pause"),
                client.property("volume"),
                client.property("path"),
            ) {
                let idle = idle.as_bool().unwrap_or(true);
                let paused = paused.as_bool().unwrap_or(false);
                effective = util::number(&volume, effective);
                path = util::text(&track_path).into();
                phase = if idle {
                    "Stopped"
                } else if paused {
                    "Paused"
                } else {
                    "Playing"
                };
                let playlist = client
                    .property("playlist-pos")
                    .ok()
                    .and_then(|v| v.as_i64());
                index = playlist.unwrap_or(0).max(0) as usize;
                if !idle {
                    match client.property("time-pos") {
                        Ok(value) if value.is_number() => position = util::number(&value, 0.),
                        _ => phase = if paused { "Paused" } else { "Loading" },
                    }
                } else if playlist.is_some_and(|n| n >= 0)
                    || util::seconds() - util::number(&saved["started"], 0.) < 60.
                {
                    phase = if paused { "Paused" } else { "Loading" };
                }
            }
            if let Ok(value) = client.property("user-data/focus-music-normalization")
                && !value.is_null()
            {
                normalization = value;
            }
        }
        let (number, track) = self.active_track(&saved, position, index, &path);
        let banner = if track.artist.is_empty() {
            track.title.clone()
        } else {
            format!("{} — {}", track.artist, track.title)
        };
        let volume = util::number(&saved["volume"], 20.).round();
        let generation = saved["generation"].as_u64().unwrap_or(0).to_string();
        json!({"text":if phase=="Stopped"{"󰎈"}else if phase=="Paused"{"󰏤"}else{"󰎆"},"phase":phase,"genre":saved["genre"],"track":number+1,"track_count":source.tracks.len(),"title":track.title,"artist":track.artist,"track_id":track.id,"banner":banner,"volume":volume,"effective_volume":effective.round(),"volume_factor":util::number(&saved["volume_factor"],1.),"generation":generation,"normalization":normalization,"catalog_revision":self.catalog.revision,"preferences":self.preferences(),"tooltip":format!("Focus music · {phase}\n{banner}\n{} · {} tracks · {:0.1} hours\n{volume}% volume\nRight-click: Play | Pause\nMiddle-click: Next · Scroll: volume",source.label,source.tracks.len(),source.duration/3600.),"class":phase.to_lowercase()})
    }
    pub fn commit(&self, saved: &mut Value) {
        let source = self.source(saved);
        let Some(order) = saved["active_order"].as_array().cloned() else {
            return;
        };
        if saved["catalog_version"] != source.catalog_version || order.is_empty() {
            return;
        }
        let Ok(mut client) = Ipc::connect(&self.paths.socket) else {
            return;
        };
        if client
            .property("idle-active")
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
        {
            return;
        }
        let Some(index) = client
            .property("playlist-pos")
            .ok()
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
        else {
            return;
        };
        let Some(number) = order.get(index).and_then(|v| v.as_u64()) else {
            return;
        };
        let Some(track) = source.tracks.get(number as usize) else {
            return;
        };
        if client.property("path").ok().as_ref().map(util::text) != Some(track.url.as_str()) {
            return;
        }
        let key = util::text(&saved["genre"]).to_string();
        if !saved["decks"].is_object() {
            saved["decks"] = json!({});
        }
        saved["decks"][&key] =
            json!({"version":source.catalog_version,"remaining":order[index+1..]});
        saved["last_track_id"] = json!(track.id);
    }
    pub fn ensure_player(&self, saved: &Value, source: &Genre) -> Result<()> {
        let needs = source
            .tracks
            .iter()
            .any(|t| crate::library::local_path(&t.url).is_none());
        let resolver = util::executable("yt-dlp");
        if needs && resolver.is_none() {
            bail!("Streaming requires yt-dlp; local music works without it");
        }
        if self.prop("idle-active").is_ok() {
            return Ok(());
        }
        let mpv = util::executable("mpv").context("Missing dependency: mpv")?;
        let _ = fs::remove_file(&self.paths.socket);
        let log = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.paths.runtime.join("player.log"))?;
        let mut command = Command::new(mpv);
        command.args([
            "--no-config",
            "--load-scripts=no",
            "--load-unsafe-playlists=no",
            "--no-video",
            "--idle=yes",
            "--keep-open=no",
            "--input-terminal=no",
            "--terminal=no",
            "--loop-file=no",
            "--loop-playlist=inf",
            "--audio-display=no",
            "--ytdl-format=bestaudio/best",
            "--pause=yes",
        ]);
        if let Some(resolver) = resolver {
            command.arg(format!(
                "--script-opts=ytdl_hook-ytdl_path={}",
                resolver.display()
            ));
        }
        command
            .arg(format!(
                "--script={}",
                self.paths.package.join("scripts/normalizer").display()
            ))
            .arg(format!(
                "--volume={}",
                util::number(&saved["volume"], 20.) * util::number(&saved["volume_factor"], 1.)
            ))
            .arg(format!(
                "--input-ipc-server={}",
                self.paths.socket.display()
            ))
            .arg("--title=Focus music")
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log));
        let mut child = util::detach(&mut command)?;
        for _ in 0..50 {
            if self.prop("idle-active").is_ok() {
                util::reap(child);
                return Ok(());
            }
            if child.try_wait()?.is_some() {
                bail!("mpv could not start");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
        bail!("Player startup timed out")
    }
    pub fn play(&self, genre: &str) -> Result<u64> {
        let source = self.catalog.get(genre).context("Unknown genre")?;
        let mut saved = self.state();
        self.commit(&mut saved);
        self.ensure_player(&saved, source)?;
        let mut order: Vec<usize> = if saved["decks"][genre]["version"] == source.catalog_version {
            saved["decks"][genre]["remaining"]
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|v| v.as_u64().map(|v| v as usize))
                        .filter(|n| *n < source.tracks.len())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if order.is_empty() {
            order = (0..source.tracks.len()).collect();
        }
        order.shuffle(&mut rand::rng());
        if order.len() > 1 && source.tracks[order[0]].id == util::text(&saved["last_track_id"]) {
            order.swap(0, 1);
        }
        let generation = util::generation();
        saved["genre"] = json!(genre);
        saved["active_order"] = json!(order);
        saved["catalog_version"] = json!(source.catalog_version);
        saved["generation"] = json!(generation);
        saved["started"] = json!(util::seconds());
        if !saved["decks"].is_object() {
            saved["decks"] = json!({});
        }
        saved["decks"][genre] = json!({"version":source.catalog_version,"remaining":order});
        self.save(&saved)?;
        let mut client = Ipc::connect(&self.paths.socket)?;
        client.command(json!(["set_property", "loop-file", "no"]))?;
        client.command(json!(["set_property", "loop-playlist", "inf"]))?;
        for (slot, index) in order.iter().enumerate() {
            let t = &source.tracks[*index];
            client.command(json!(["loadfile",t.url,if slot==0{"replace"}else{"append"},-1,{"start":t.start.to_string(),"end":t.end.to_string(),"force-media-title":if t.artist.is_empty(){t.title.clone()}else{format!("{} — {}",t.artist,t.title)}}]))?;
        }
        client.command(json!(["set_property", "pause", false]))?;
        Ok(generation)
    }
    pub fn restart_player(&self) -> Result<Value> {
        let _lock = util::Lock::acquire(&self.paths.runtime.join("control.lock"), false)?;
        if self.prop("idle-active").unwrap_or(json!(true)) == true {
            return Ok(self.status());
        }
        let saved = self.state();
        let source = self.source(&saved);
        let order = saved["active_order"]
            .as_array()
            .context("Cannot restore an unindexed session")?
            .iter()
            .map(|v| {
                v.as_u64()
                    .context("Invalid queue entry")
                    .map(|v| v as usize)
            })
            .collect::<Result<Vec<_>>>()?;
        if order.is_empty()
            || saved["catalog_version"] != source.catalog_version
            || order.iter().any(|i| *i >= source.tracks.len())
        {
            bail!("Cannot restore an unindexed session");
        }
        let index = self.prop("playlist-pos")?.as_u64().unwrap_or(0) as usize;
        if index >= order.len() {
            bail!("Cannot restore an invalid playlist position");
        }
        let track = &source.tracks[order[index]];
        let position = util::number(&self.prop("time-pos").unwrap_or(Value::Null), track.start)
            .clamp(track.start, (track.end - 0.001).max(track.start));
        let paused = self.prop("pause")?.as_bool().unwrap_or(false);
        self.command(json!(["quit"]))?;
        for _ in 0..100 {
            if self.prop("idle-active").is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if self.prop("idle-active").is_ok() {
            bail!("Previous player did not exit");
        }
        self.ensure_player(&saved, source)?;
        let mut client = Ipc::connect(&self.paths.socket)?;
        client.command(json!(["set_property", "pause", true]))?;
        for (slot, number) in order.iter().enumerate() {
            let track = &source.tracks[*number];
            client.command(json!(["loadfile", track.url, if slot == 0 {"replace"} else {"append"}, -1,
                {"start":track.start.to_string(),"end":track.end.to_string(),"force-media-title":format!("{} — {}", track.artist, track.title)}]))?;
        }
        client.command(json!(["set_property", "playlist-pos", index]))?;
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while !client.property("time-pos").is_ok_and(|v| v.is_number()) {
            if std::time::Instant::now() >= deadline {
                bail!("Restored player is still loading; the queue is preserved and paused");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        client.command(json!(["seek", position, "absolute+exact"]))?;
        client.command(json!(["set_property", "pause", paused]))?;
        Ok(self.status())
    }

    pub fn sync_other_media(&self, active: bool) -> Result<()> {
        let _lock = util::Lock::acquire(&self.paths.runtime.join("control.lock"), false)?;
        self.sync_locked(active)
    }
    pub fn sync_locked(&self, active: bool) -> Result<()> {
        let mut saved = self.state();
        let settings = self.preferences();
        let mode = util::text(&settings["otherMedia"]);
        let factor = if active {
            match mode {
                "duck-strong" => 0.4,
                "duck-gentle" => 0.75,
                _ => 1.,
            }
        } else {
            1.
        };
        let mut changed = util::number(&saved["volume_factor"], 1.) != factor;
        saved["volume_factor"] = json!(factor);
        if let Ok(paused) = self.prop("pause") {
            if active && mode == "pause-resume" {
                if paused == false && self.prop("idle-active").unwrap_or(json!(true)) == false {
                    self.command(json!(["set_property", "pause", true]))?;
                    saved["auto_paused"] = json!(true);
                    changed = true;
                }
            } else if saved["auto_paused"] == true {
                self.command(json!(["set_property", "pause", false]))?;
                saved["auto_paused"] = json!(false);
                changed = true;
            }
            if changed {
                let _ = self.command(json!([
                    "set_property",
                    "volume",
                    util::number(&saved["volume"], 20.) * factor
                ]));
            }
        } else if saved["auto_paused"] == true {
            saved["auto_paused"] = json!(false);
            changed = true;
        }
        if changed {
            self.save(&saved)?;
        }
        Ok(())
    }
    pub fn configure(&self, changes: &Value) -> Result<Value> {
        preferences::configure(&self.paths, changes)
    }
    pub fn action(&self, action: &str, args: &[String]) -> Result<Option<Value>> {
        let mut generation = None;
        let response;
        {
            let _lock = util::Lock::acquire(&self.paths.runtime.join("control.lock"), false)?;
            let preserve = args.first().is_some_and(|s| s == "keep-pause")
                && self.status()["phase"] == "Paused";
            if ["play", "pause-set", "toggle", "skip", "previous", "stop"].contains(&action) {
                let mut saved = self.state();
                saved["auto_paused"] = json!(false);
                self.save(&saved)?;
            }
            response = match action {
                "play" => {
                    generation = Some(self.play(args.first().context("Missing genre")?)?);
                    None
                }
                "pause-set" => {
                    let paused = util::finite_arg(args.first(), "pause state")? != 0.;
                    if self.status()["phase"] == "Stopped" {
                        if !paused {
                            generation = Some(self.play(util::text(&self.state()["genre"]))?);
                        }
                    } else {
                        self.command(json!(["set_property", "pause", paused]))?;
                    }
                    None
                }
                "toggle" => {
                    if self.status()["phase"] == "Stopped" {
                        generation = Some(self.play(util::text(&self.state()["genre"]))?);
                    } else {
                        self.command(json!(["cycle", "pause"]))?;
                    }
                    None
                }
                "skip" => {
                    let current = self.status();
                    if current["phase"] == "Stopped"
                        || ["legacy", "unknown"].contains(&util::text(&current["track_id"]))
                    {
                        generation = Some(self.play(util::text(&self.state()["genre"]))?);
                    } else {
                        self.command(json!(["playlist-next", "force"]))?;
                        self.command(json!(["set_property", "pause", false]))?;
                    }
                    None
                }
                "previous" => {
                    let current = self.status();
                    if current["phase"] == "Stopped" || current["track_id"] == "unknown" {
                        generation = Some(self.play(util::text(&self.state()["genre"]))?);
                    } else {
                        let index = self.prop("playlist-pos")?.as_u64().unwrap_or(0);
                        if index > 0 {
                            self.command(json!(["playlist-prev", "force"]))?;
                        } else {
                            let saved = self.state();
                            let track = self.selected_track(&saved, 0)?;
                            self.command(json!(["seek", track.start, "absolute+exact"]))?;
                        }
                        self.command(json!(["set_property", "pause", false]))?;
                    }
                    None
                }
                "stop" => {
                    let mut saved = self.state();
                    self.commit(&mut saved);
                    saved["started"] = json!(0);
                    self.save(&saved)?;
                    let _ = self.command(json!(["quit"]));
                    None
                }
                "volume" | "volume-set" => {
                    let mut saved = self.state();
                    let amount = util::finite_arg(args.first(), "volume")?;
                    let volume = if action == "volume-set" {
                        amount
                    } else {
                        util::number(&saved["volume"], 20.) + amount
                    };
                    let volume = volume.clamp(0., 100.);
                    saved["volume"] = json!(volume);
                    self.save(&saved)?;
                    let _ = self.command(json!([
                        "set_property",
                        "volume",
                        volume * util::number(&saved["volume_factor"], 1.)
                    ]));
                    Some(json!({"volume":volume}))
                }
                "media-sync" => {
                    self.sync_locked(util::finite_arg(args.first(), "media state")? != 0.)?;
                    None
                }
                _ => bail!("Unknown action: {action}"),
            };
            if preserve && ["skip", "previous"].contains(&action) {
                self.command(json!(["set_property", "pause", true]))?;
            }
        }
        if let Some(generation) = generation {
            for _ in 0..120 {
                if util::stopping() || self.state()["generation"].as_u64() != Some(generation) {
                    break;
                }
                if self.prop("idle-active").ok() == Some(json!(false))
                    && self.prop("time-pos").is_ok_and(|v| v.is_number())
                {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        if action == "pause-set" {
            Ok(Some(self.status()))
        } else {
            Ok(response)
        }
    }
    pub fn selected_track<'a>(&'a self, saved: &Value, index: usize) -> Result<&'a Track> {
        let source = self.source(saved);
        if saved["catalog_version"] != source.catalog_version {
            bail!("Playlist is not indexed");
        }
        let track = saved["active_order"][index]
            .as_u64()
            .context("Playlist is not indexed")? as usize;
        source.tracks.get(track).context("Playlist is not indexed")
    }
}
