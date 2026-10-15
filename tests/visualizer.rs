mod common;
use common::Fixture;
use omarchy_focus_music::util;
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
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
fn script(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}
#[test]
fn capture_is_target_locked_and_stops_on_node_loss_even_with_continuous_samples() {
    let fixture = Fixture::new();
    let p = &fixture.controller.paths;
    let tools = fixture._root.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let graph = tools.join("graph.json");
    util::atomic_json(&graph, &json!([{"id":40,"type":"PipeWire:Interface:Node","info":{"props":{"application.process.id":std::process::id(),"media.class":"Stream/Output/Audio","object.serial":400}}}])).unwrap();
    script(
        &tools.join("pw-dump"),
        "#!/bin/sh\ncat \"$FOCUS_TEST_GRAPH\"\n",
    );
    script(
        &tools.join("pw-cat"),
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FOCUS_TEST_ARGS\"\nwhile :; do dd if=/dev/zero bs=4096 count=1 2>/dev/null; sleep 0.02; done\n",
    );
    let frames = tools.join("frames");
    let args = tools.join("args");
    let mut child = Process(
        Command::new(env!("CARGO_BIN_EXE_focus-music"))
            .arg("visualizer")
            .env("FOCUS_MUSIC_PACKAGE", &p.package)
            .env("FOCUS_MUSIC_RUNTIME", &p.runtime)
            .env("FOCUS_MUSIC_STATE", &p.state)
            .env("FOCUS_MUSIC_PREFERENCES", &p.preferences)
            .env("FOCUS_TEST_GRAPH", &graph)
            .env("FOCUS_TEST_ARGS", &args)
            .env(
                "PATH",
                format!("{}:{}", tools.display(), std::env::var("PATH").unwrap()),
            )
            .stdout(fs::File::create(&frames).unwrap())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let end = Instant::now() + Duration::from_secs(5);
    while !fs::read_to_string(&frames).unwrap().contains("\"active\"") {
        assert!(Instant::now() < end, "No active spectrum frames");
        std::thread::sleep(Duration::from_millis(20));
    }
    let args = fs::read_to_string(args).unwrap();
    assert!(args.contains("--target\n400\n"));
    for property in [
        "node.dont-fallback=true",
        "node.dont-reconnect=true",
        "node.dont-move=true",
        "node.linger=false",
    ] {
        assert!(args.contains(property), "Missing {property}");
    }
    // Past the old two-second track-transition check, with unchanged playlist-pos.
    std::thread::sleep(Duration::from_secs(3));
    assert!(child.0.try_wait().unwrap().is_none());
    util::atomic_json(&graph, &json!([])).unwrap();
    let end = Instant::now() + Duration::from_secs(3);
    while child.0.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < end,
            "Continuous input bypassed node identity checks"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(child.0.wait().unwrap().success());
    // No selected node: fail closed before starting any capture command.
    fs::remove_file(tools.join("args")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_focus-music"))
        .arg("visualizer")
        .env("FOCUS_MUSIC_PACKAGE", &p.package)
        .env("FOCUS_MUSIC_RUNTIME", &p.runtime)
        .env("FOCUS_MUSIC_STATE", &p.state)
        .env("FOCUS_MUSIC_PREFERENCES", &p.preferences)
        .env("FOCUS_TEST_GRAPH", &graph)
        .env("FOCUS_TEST_ARGS", tools.join("args"))
        .env(
            "PATH",
            format!("{}:{}", tools.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("unavailable"));
    assert!(
        !tools.join("args").exists(),
        "Capture started without a player node"
    );
}
