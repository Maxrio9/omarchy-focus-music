use omarchy_focus_music::util;
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(6);
    while !condition() {
        assert!(Instant::now() < end, "Private PipeWire condition timed out");
        std::thread::sleep(Duration::from_millis(40));
    }
}
struct Session(PathBuf);
impl Session {
    fn command(&self, program: &str) -> Command {
        let mut c = Command::new(program);
        c.env("XDG_RUNTIME_DIR", &self.0)
            .env("PIPEWIRE_RUNTIME_DIR", &self.0)
            .env("PIPEWIRE_REMOTE", "pipewire-0")
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_STATE_HOME", self.0.join("state"))
            .env("XDG_CACHE_HOME", self.0.join("cache"))
            .env_remove("PIPEWIRE_NODE")
            .env_remove("PIPEWIRE_PROPS")
            .stdin(Stdio::null());
        c
    }
    fn start(&self, mut command: Command, name: &str) -> Process {
        Process(
            command
                .stdout(fs::File::create(self.0.join(format!("{name}.out"))).unwrap())
                .stderr(fs::File::create(self.0.join(format!("{name}.log"))).unwrap())
                .spawn()
                .unwrap(),
        )
    }
    fn graph(&self) -> Vec<Value> {
        let output =
            util::capture(&mut self.command("pw-dump"), Duration::from_secs(3), None).unwrap();
        assert!(output.success);
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn capture(&self, target: &str, safe: bool, name: &str) -> Process {
        let mut c = self.command("pw-cat");
        c.args([
            "--record",
            "--raw",
            "--format",
            "f32",
            "--rate",
            "8000",
            "--channels",
            "1",
            "--target",
            target,
        ]);
        if safe {
            c.args(["--properties", "node.name=focus-test-capture node.dont-fallback=true node.dont-reconnect=true node.dont-move=true node.linger=false"]);
        }
        c.arg("-");
        self.start(c, name)
    }
    fn bytes(&self, name: &str) -> u64 {
        fs::metadata(self.0.join(format!("{name}.out")))
            .unwrap()
            .len()
    }
}
#[test]
fn stock_policy_does_not_fallback_to_a_synthetic_microphone() {
    if std::env::var_os("FOCUS_PRIVATE_PIPEWIRE").is_none() {
        if [
            "pipewire",
            "wireplumber",
            "pw-cat",
            "pw-dump",
            "dbus-run-session",
        ]
        .iter()
        .any(|tool| util::executable(tool).is_none())
        {
            assert!(
                std::env::var_os("FOCUS_REQUIRE_PIPEWIRE_TEST").is_none(),
                "Required PipeWire test tools missing"
            );
            eprintln!(
                "Private PipeWire regression requires PipeWire and WirePlumber 0.5+; skipped"
            );
            return;
        }
        let help = Command::new("wireplumber").arg("--help").output().unwrap();
        if !String::from_utf8_lossy(&help.stdout).contains("--profile") {
            assert!(
                std::env::var_os("FOCUS_REQUIRE_PIPEWIRE_TEST").is_none(),
                "Required WirePlumber policy profile missing"
            );
            eprintln!(
                "WirePlumber 0.5+ policy profile unavailable; private PipeWire regression skipped"
            );
            return;
        }
        let result = util::capture(
            Command::new("dbus-run-session")
                .args(["--"])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "stock_policy_does_not_fallback_to_a_synthetic_microphone",
                    "--nocapture",
                ])
                .env("FOCUS_PRIVATE_PIPEWIRE", "1"),
            Duration::from_secs(35),
            None,
        )
        .unwrap();
        assert!(
            result.success,
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        println!(
            "Private stock-policy test reproduced legacy synthetic fallback, rejected missing targets and exited on target removal"
        );
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let session = Session(root.path().join("runtime"));
    util::private_dir(&session.0).unwrap();
    let config = session.0.join("pipewire.conf");
    // No hardware factories or monitors. Every source/sink below is synthetic.
    fs::write(&config,r#"
context.properties = { core.daemon = true core.name = pipewire-0 }
context.spa-libs = { support.* = support/libspa-support audio.convert.* = audioconvert/libspa-audioconvert audiotestsrc = audiotestsrc/libspa-audiotestsrc }
context.modules = [ { name = libpipewire-module-protocol-native } { name = libpipewire-module-metadata } { name = libpipewire-module-spa-node-factory } { name = libpipewire-module-client-node } { name = libpipewire-module-adapter } { name = libpipewire-module-link-factory } { name = libpipewire-module-access } ]
context.objects = [
{ factory = spa-node-factory args = { factory.name = support.node.driver node.name = Dummy-Driver node.group = pipewire.dummy priority.driver = 200000 } }
{ factory = adapter args = { factory.name = audiotestsrc node.name = focus-synthetic-microphone media.class = Audio/Source audio.channels = 1 audio.position = [ MONO ] node.param.Props = { live = true freq = 1000 volume = 0.2 } } }
{ factory = adapter args = { factory.name = support.null-audio-sink node.name = focus-test-sink media.class = Audio/Sink audio.channels = 1 audio.position = [ MONO ] } }
]
"#).unwrap();
    let mut c = session.command("pipewire");
    c.arg("-c").arg(config);
    let _pipewire = session.start(c, "pipewire");
    wait(|| session.0.join("pipewire-0").exists());
    // This stock profile loads linking policy without ALSA/Bluetooth/camera monitors.
    let mut c = session.command("wireplumber");
    c.args(["--profile", "policy"]);
    let _policy = session.start(c, "wireplumber");
    wait(|| {
        session
            .graph()
            .iter()
            .any(|n| n["props"]["metadata.name"] == "default")
    });
    assert!(
        !session
            .graph()
            .iter()
            .any(|n| n["info"]["props"].get("device.api").is_some())
    );
    let mut legacy = session.capture("999999", false, "legacy");
    wait(|| session.bytes("legacy") > 0);
    legacy.0.kill().unwrap();
    legacy.0.wait().unwrap();
    let mut missing = session.capture("999999", true, "missing");
    wait(|| missing.0.try_wait().unwrap().is_some());
    assert_eq!(session.bytes("missing"), 0);
    let mut c = session.command("pw-cat");
    // Older pw-cat raw mode reads stdin even when given a filename.
    let audio = session.0.join("player.raw");
    fs::write(&audio, vec![0u8; 8000 * 4 * 60]).unwrap();
    c.args([
        "--playback",
        "--raw",
        "--format",
        "f32",
        "--rate",
        "8000",
        "--channels",
        "1",
        "--target",
        "focus-test-sink",
        "--properties",
        "node.name=focus-test-player",
    ]);
    c.arg("-").stdin(fs::File::open(audio).unwrap());
    let mut player = session.start(c, "player");
    let mut serial = String::new();
    wait(|| {
        if let Some(node) = session
            .graph()
            .iter()
            .find(|n| n["info"]["props"]["node.name"] == "focus-test-player")
        {
            serial = node["info"]["props"]["object.serial"].to_string();
            true
        } else {
            false
        }
    });
    let mut capture = session.capture(&serial, true, "protected");
    wait(|| session.bytes("protected") > 0);
    assert!(
        player.0.try_wait().unwrap().is_none(),
        "Private player exited: {}",
        fs::read_to_string(session.0.join("player.log")).unwrap()
    );
    assert!(
        capture.0.try_wait().unwrap().is_none(),
        "Private capture exited: stderr={} stdout={:?}",
        fs::read_to_string(session.0.join("protected.log")).unwrap(),
        fs::read(session.0.join("protected.out")).unwrap()
    );
    let graph = session.graph();
    let node = graph
        .iter()
        .find(|n| n["info"]["props"]["node.name"] == "focus-test-capture")
        .unwrap_or_else(|| panic!("Capture stream missing from private graph"));
    for property in [
        "node.dont-fallback",
        "node.dont-reconnect",
        "node.dont-move",
    ] {
        assert_eq!(node["info"]["props"][property], true, "Missing {property}");
    }
    player.0.kill().unwrap();
    player.0.wait().unwrap();
    wait(|| capture.0.try_wait().unwrap().is_some());
    let ended = session.bytes("protected");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(session.bytes("protected"), ended);
}
