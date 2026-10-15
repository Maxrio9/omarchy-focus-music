use omarchy_focus_music::{catalog::Catalog, library, normalizer, util};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
};

#[test]
fn catalog_rejects_device_and_player_protocols_before_playback() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("sources.json");
    for url in [
        "av://alsa:default",
        "av://pulse:default",
        "lavfi://amovie=/dev/zero",
        "edl://foo",
        "memory://foo",
        "fd://0",
        "-",
        "relative.wav",
        "file://remote/share/a.wav",
        "https:///bad",
        "https://example.org/a\nheader",
        "/dev/zero",
    ] {
        util::atomic_json(&source, &json!({"test":{"label":"Test","catalog_version":"1","duration":1,"tracks":[{"id":"test","title":"Test","url":url,"start":0,"end":1,"duration":1}]}})).unwrap();
        assert!(
            Catalog::load(&source, &root.path().join("absent.json")).is_err(),
            "Accepted {url}"
        );
    }
    for url in [
        "https://example.org/music",
        "http://localhost:8080/a.ogg",
        "/missing music/song.wav",
        "file:///missing%20music/song.wav",
    ] {
        assert!(library::validate_source(url).is_ok(), "Rejected {url}");
    }
    assert!(normalizer::measure_source("av://alsa:default", 0., 1., None).is_err());
    assert!(normalizer::measure_source("/dev/zero", 0., 1., None).is_err());
}
#[test]
fn private_directory_and_locks_reject_symlinks_without_changing_targets() {
    let root = tempfile::tempdir().unwrap();
    let victim = root.path().join("victim");
    fs::create_dir(&victim).unwrap();
    fs::set_permissions(&victim, fs::Permissions::from_mode(0o755)).unwrap();
    let runtime = root.path().join("runtime");
    symlink(&victim, &runtime).unwrap();
    assert!(util::private_dir(&runtime).is_err());
    assert_eq!(fs::metadata(&victim).unwrap().mode() & 0o777, 0o755);
    let private = root.path().join("private");
    util::private_dir(&private).unwrap();
    assert_eq!(fs::metadata(&private).unwrap().mode() & 0o777, 0o700);
    let contents = root.path().join("contents");
    fs::write(&contents, "keep").unwrap();
    let lock = private.join("control.lock");
    symlink(&contents, &lock).unwrap();
    assert!(util::Lock::acquire(&lock, false).is_err());
    assert_eq!(fs::read_to_string(contents).unwrap(), "keep");
}
#[test]
fn json_writes_do_not_follow_predictable_temporary_or_destination_symlinks() {
    let root = tempfile::tempdir().unwrap();
    let victim = root.path().join("victim");
    fs::write(&victim, "keep").unwrap();
    let target = root.path().join("state.json");
    symlink(
        &victim,
        target.with_extension(format!("{}.tmp", std::process::id())),
    )
    .unwrap();
    symlink(&victim, &target).unwrap();
    util::atomic_json(&target, &json!({"ok":true})).unwrap();
    assert_eq!(fs::read_to_string(&victim).unwrap(), "keep");
    assert_eq!(util::read_json(&target).unwrap(), json!({"ok":true}));
    assert_eq!(fs::metadata(&target).unwrap().mode() & 0o777, 0o600);
    // Concurrent RPC workers in one process must not share a temporary filename.
    std::thread::scope(|scope| {
        for value in 0..16 {
            let path = &target;
            scope.spawn(move || util::atomic_json(path, &json!({"value":value})).unwrap());
        }
    });
    assert!(util::read_json(&target).unwrap()["value"].is_number());
}

#[test]
fn compact_plugin_paths_cannot_escape_through_symlinks() {
    let root = tempfile::tempdir().unwrap();
    let plugin = root.path().join("plugin");
    fs::create_dir(&plugin).unwrap();
    fs::write(root.path().join("outside.qml"), "keep").unwrap();
    fs::write(plugin.join("inside.qml"), "ok").unwrap();
    symlink(root.path().join("outside.qml"), plugin.join("escape.qml")).unwrap();
    assert!(util::checked_relative(&plugin, "escape.qml").is_err());
    assert!(util::checked_relative(&plugin, "../outside.qml").is_err());
    assert!(util::checked_relative(&plugin, "inside.qml").is_ok());
}

mod common;
#[test]
fn online_resolver_cannot_redirect_loudness_analysis_to_a_local_file() {
    let fixture = common::Fixture::new();
    let p = &fixture.controller.paths;
    fixture.controller.play("piano").unwrap();
    let source = p.config_home.join("sources.json");
    let mut catalog = util::read_json(&source).unwrap();
    for genre in catalog.as_object_mut().unwrap().values_mut() {
        for track in genre["tracks"].as_array_mut().unwrap() {
            track["url"] = json!("https://example.org/music");
        }
    }
    util::atomic_json(&source, &catalog).unwrap();
    let tools = fixture._root.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let local = tools.join("private.wav");
    fs::write(&local, "private contents").unwrap();
    for (name, script) in [
        (
            "yt-dlp",
            "#!/bin/sh\nprintf '%s\\n' \"$FOCUS_TEST_RESOLVED\"\n",
        ),
        (
            "ffmpeg",
            "#!/bin/sh\ntouch \"$FOCUS_TEST_FFMPEG\"\nexit 1\n",
        ),
    ] {
        fs::write(tools.join(name), script).unwrap();
        fs::set_permissions(tools.join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let marker = tools.join("ffmpeg-executed");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_focus-music"))
        .args([
            "normalize",
            "current",
            "0",
            &fixture.controller.state()["generation"].to_string(),
        ])
        .env("FOCUS_MUSIC_PACKAGE", &p.package)
        .env("FOCUS_MUSIC_RUNTIME", &p.runtime)
        .env("FOCUS_MUSIC_CONFIG", &source)
        .env("FOCUS_MUSIC_LOCAL_CONFIG", &p.local)
        .env("FOCUS_MUSIC_STATE", &p.state)
        .env("FOCUS_MUSIC_CACHE", &p.cache)
        .env("FOCUS_TEST_RESOLVED", format!("file://{}", local.display()))
        .env("FOCUS_TEST_FFMPEG", &marker)
        .env(
            "PATH",
            format!("{}:{}", tools.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("unavailable"));
    assert!(
        !marker.exists(),
        "Resolver redirected FFmpeg to a local file"
    );
}
