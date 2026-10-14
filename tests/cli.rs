use omarchy_focus_music::{compact, util};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn command(root: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_focus-music"));
    command
        .args(args)
        .env("FOCUS_MUSIC_PACKAGE", env!("CARGO_MANIFEST_DIR"))
        .env("FOCUS_MUSIC_RUNTIME", root.join("runtime"))
        .env("FOCUS_MUSIC_STATE", root.join("state.json"))
        .env("FOCUS_MUSIC_LOCAL_CONFIG", root.join("local.json"))
        .env("FOCUS_MUSIC_PREFERENCES", root.join("preferences.json"))
        .env(
            "FOCUS_MUSIC_CONFIG",
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/sources.json"),
        )
        .output()
        .unwrap()
}
#[test]
fn cli_and_user_storage_work_from_paths_with_spaces_and_unicode() {
    let root = tempfile::Builder::new()
        .prefix("New user ü ")
        .tempdir()
        .unwrap();
    for (action, arg, expected) in [
        ("volume-set", "100", 100.),
        ("volume-set", "-5", 0.),
        ("volume-set", "30", 30.),
        ("volume", "5", 35.),
    ] {
        let output = command(root.path(), &["music", action, arg]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["volume"].as_f64(), Some(expected));
    }
    let output = command(root.path(), &["music", "genres"]);
    let genres: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(genres.as_array().unwrap().len(), 5);
    assert_eq!(genres[0]["value"], "lofi-hiphop");
    let output = command(root.path(), &["music", "status"]);
    let status: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["phase"], "Stopped");
    assert_eq!(status["volume"].as_f64(), Some(35.));
    assert!(root.path().join("state.json").exists());
}
#[test]
fn empty_unset_and_custom_xdg_paths_are_supported() {
    let root = tempfile::tempdir().unwrap();
    for mode in ["unset", "empty", "custom"] {
        let home = root.path().join(format!("Home ü {mode}"));
        fs::create_dir_all(&home).unwrap();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_focus-music"));
        cmd.args(["music", "volume-set", "30"])
            .env("HOME", &home)
            .env("FOCUS_MUSIC_PACKAGE", env!("CARGO_MANIFEST_DIR"))
            .env(
                "XDG_RUNTIME_DIR",
                root.path().join(format!("runtime-{mode}")),
            );
        for key in [
            "FOCUS_MUSIC_CONFIG",
            "FOCUS_MUSIC_STATE",
            "FOCUS_MUSIC_RUNTIME",
            "FOCUS_MUSIC_LOCAL_CONFIG",
            "FOCUS_MUSIC_PREFERENCES",
            "XDG_CONFIG_HOME",
            "XDG_STATE_HOME",
        ] {
            cmd.env_remove(key);
            if mode == "empty" {
                cmd.env(key, "");
            }
        }
        let state = if mode == "custom" {
            let config = root.path().join("custom config");
            let state = root.path().join("custom state");
            cmd.env("XDG_CONFIG_HOME", config)
                .env("XDG_STATE_HOME", &state);
            state
        } else {
            home.join(".local/state")
        };
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(state.join("focus-music/state.json").is_file());
    }
}
#[test]
fn compact_cleanup_touches_only_recognized_user_clones() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let known = config.join("omarchy/plugins/custom.bar");
    let unrelated = config.join("omarchy/plugins/other.bar");
    fs::create_dir_all(&known).unwrap();
    fs::create_dir_all(&unrelated).unwrap();
    let manifest = json!({"omarchy":{"clonedFrom":"omarchy.bar"},"entryPoints":{"bar":"Bar.qml"}});
    util::atomic_json(&known.join("manifest.json"), &manifest).unwrap();
    util::atomic_json(&unrelated.join("manifest.json"), &manifest).unwrap();
    fs::write(
        known.join("Bar.qml"),
        format!("{}{}", compact::RIGHT_COMPACT, compact::HOVER_FUNCTION),
    )
    .unwrap();
    fs::write(unrelated.join("Bar.qml"), "Unrelated custom bar").unwrap();
    for action in ["--disable", "--restore"] {
        let output = Command::new(env!("CARGO_BIN_EXE_focus-music"))
            .args(["compact", action])
            .env("FOCUS_MUSIC_PACKAGE", env!("CARGO_MANIFEST_DIR"))
            .env("XDG_CONFIG_HOME", &config)
            .env("XDG_STATE_HOME", root.path().join("state"))
            .env("FOCUS_MUSIC_RUNTIME", root.path().join("runtime"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let source = fs::read_to_string(known.join("Bar.qml")).unwrap();
        assert!(source.contains(if action == "--disable" {
            compact::RIGHT_ORIGINAL
        } else {
            compact::RIGHT_COMPACT
        }));
        assert_eq!(
            fs::read_to_string(unrelated.join("Bar.qml")).unwrap(),
            "Unrelated custom bar"
        );
    }
    assert!(
        fs::read_dir(root.path().join("state/focus-music/backups"))
            .unwrap()
            .count()
            >= 2
    );
}

#[test]
fn placement_commands_preserve_native_bar_routes() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let tools = root.path().join("tools");
    fs::create_dir_all(&tools).unwrap();
    let fake = tools.join("omarchy");
    fs::write(
        &fake,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FOCUS_ARGV_LOG\"\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let log = root.path().join("argv.log");
    let invoke = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_focus-music"))
            .args(args)
            .env("FOCUS_MUSIC_PACKAGE", env!("CARGO_MANIFEST_DIR"))
            .env("FOCUS_MUSIC_RUNTIME", root.path().join("runtime"))
            .env(
                "FOCUS_MUSIC_PREFERENCES",
                root.path().join("preferences.json"),
            )
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("XDG_STATE_HOME", root.path().join("state"))
            .env("FOCUS_ARGV_LOG", &log)
            .env(
                "PATH",
                format!("{}:{}", tools.display(), std::env::var("PATH").unwrap()),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    for (side, position) in [("left", "--before"), ("right", "--after")] {
        invoke(&["music", "place", side]);
        assert_eq!(
            fs::read_to_string(&log)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            vec![
                "bar",
                "move",
                "maxrio9.focus-music",
                "--section",
                "center",
                position,
                "omarchy.clock"
            ]
        );
    }
    let output = invoke(&["music", "settings-set", r#"{"placement":"right"}"#]);
    let settings: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(settings["barDisplay"], "icon");
    assert_eq!(
        fs::read_to_string(&log)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "bar",
            "move",
            "maxrio9.focus-music",
            "--section",
            "right",
            "--index",
            "0"
        ]
    );
}
