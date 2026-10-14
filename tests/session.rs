use omarchy_focus_music::{
    controller::{Controller, Paths},
    util,
};
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
fn wav(path: &std::path::Path) {
    let rate = 16000usize;
    let count = rate * 20;
    let mut data = Vec::new();
    for i in 0..count {
        let sample =
            (3000. * (2. * std::f64::consts::PI * 440. * i as f64 / rate as f64).sin()) as i16;
        data.extend(sample.to_le_bytes());
    }
    let mut file = fs::File::create(path).unwrap();
    file.write_all(b"RIFF").unwrap();
    file.write_all(&((data.len() + 36) as u32).to_le_bytes())
        .unwrap();
    file.write_all(b"WAVEfmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&(rate as u32).to_le_bytes()).unwrap();
    file.write_all(&(rate as u32 * 2).to_le_bytes()).unwrap();
    file.write_all(&2u16.to_le_bytes()).unwrap();
    file.write_all(&16u16.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&(data.len() as u32).to_le_bytes()).unwrap();
    file.write_all(&data).unwrap();
}
#[test]
fn upgrades_restore_real_mpv_queue_position_pause_and_volume() {
    if std::env::var_os("FOCUS_REAL_MPV_TEST").is_none() {
        let Some(mpv) = util::executable("mpv") else {
            return;
        };
        let version = Command::new(&mpv).arg("--version").output().unwrap();
        let version = String::from_utf8_lossy(&version.stdout);
        let minor = version
            .split_whitespace()
            .nth(1)
            .unwrap_or("0.0")
            .split('.')
            .nth(1)
            .unwrap_or("0")
            .parse::<u32>()
            .unwrap_or(0);
        if minor < 38 {
            eprintln!("Real session upgrade test requires mpv 0.38+; skipped");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("tools");
        fs::create_dir_all(&path).unwrap();
        let shim = path.join("mpv");
        fs::write(
            &shim,
            format!("#!/bin/sh\nexec '{}' --ao=null \"$@\"\n", mpv.display()),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(shim, fs::Permissions::from_mode(0o755)).unwrap();
        let current = std::env::var("PATH").unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "upgrades_restore_real_mpv_queue_position_pause_and_volume",
                "--nocapture",
            ])
            .env("FOCUS_REAL_MPV_TEST", root.path())
            .env("PATH", format!("{}:{current}", path.display()))
            .env("FOCUS_MUSIC_BINARY", env!("CARGO_BIN_EXE_focus-music"))
            .env("FOCUS_MUSIC_NORMALIZE", "0")
            .env("FOCUS_MUSIC_PACKAGE", env!("CARGO_MANIFEST_DIR"))
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
    let root = PathBuf::from(std::env::var_os("FOCUS_REAL_MPV_TEST").unwrap());
    let audio = root.join("Music ü.wav");
    wav(&audio);
    let config_home = root.join("config/focus-music");
    let runtime = root.join("runtime");
    util::private_dir(&runtime).unwrap();
    let tracks=(0..4).map(|i|json!({"id":format!("local-{i}"),"artist":"Test artist","title":format!("Segment {i}"),"url":audio.to_string_lossy(),"start":i*4,"end":i*4+4,"duration":4})).collect::<Vec<_>>();
    util::atomic_json(&config_home.join("sources.json"),&json!({"piano":{"label":"Piano","catalog_version":"local-v1","duration":16,"tracks":tracks}})).unwrap();
    let paths = Paths {
        package: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        config_home,
        local: root.join("local.json"),
        preferences: root.join("preferences.json"),
        socket: runtime.join("mpv.sock"),
        runtime,
        state: root.join("state.json"),
        shell: root.join("shell.json"),
        cache: root.join("cache"),
    };
    let p = Controller::new(paths).unwrap();
    struct Cleanup<'a>(&'a Controller);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.command(json!(["quit"]));
        }
    }
    let _cleanup = Cleanup(&p);
    p.play("piano").unwrap();
    p.action("pause-set", &["1".into()]).unwrap();
    p.action("skip", &["keep-pause".into()]).unwrap();
    let start = Instant::now();
    while !p.prop("time-pos").is_ok_and(|v| v.is_number()) {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
    }
    let saved = p.state();
    let track = p.selected_track(&saved, 1).unwrap();
    let position = track.start + 1.;
    p.command(json!(["seek", position, "absolute+exact"]))
        .unwrap();
    p.action("volume-set", &["37".into()]).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let old = p.prop("time-pos").unwrap().as_f64().unwrap();
    let status = p.restart_player().unwrap();
    assert_eq!(status["phase"], "Paused");
    assert_eq!(p.prop("playlist-pos").unwrap().as_u64(), Some(1));
    assert_eq!(p.prop("volume").unwrap().as_f64(), Some(37.));
    assert_eq!(p.state()["generation"], saved["generation"]);
    assert_eq!(p.state()["active_order"], saved["active_order"]);
    let new = p.prop("time-pos").unwrap().as_f64().unwrap();
    assert!((new - old).abs() < 0.1, "Position moved: {old} -> {new}");
    println!(
        "Real offline mpv migration retained queue, generation, segment position, paused state, and volume"
    );
}
