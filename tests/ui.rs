mod common;
use common::Fixture;
use omarchy_focus_music::util;
use std::{fs, process::Command, time::Duration};
#[test]
fn unchanged_ui_controls_native_backend() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none()
        || util::executable("quickshell").is_none()
        || !std::path::Path::new("/usr/share/omarchy/shell/Commons").is_dir()
    {
        eprintln!("Omarchy Wayland UI unavailable; UI integration skipped");
        return;
    }
    let f = Fixture::new();
    f.controller.play("piano").unwrap();
    f.controller.action("skip", &[]).unwrap();
    f.controller.action("volume-set", &["25".into()]).unwrap();
    let directory = f._root.path().join("qml");
    fs::create_dir_all(&directory).unwrap();
    std::os::unix::fs::symlink(
        "/usr/share/omarchy/shell/Commons",
        directory.join("Commons"),
    )
    .unwrap();
    std::os::unix::fs::symlink("/usr/share/omarchy/shell/Ui", directory.join("Ui")).unwrap();
    let source = serde_json::to_string(&format!(
        "file://{}/BarWidget.qml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let qml = include_str!("fixtures/ui.qml").replace("FOCUS_WIDGET_SOURCE", &source);
    let file = directory.join("shell.qml");
    fs::write(&file, qml).unwrap();
    let p = &f.controller.paths;
    let mut command = Command::new("dbus-run-session");
    command
        .arg("--")
        .args(["quickshell", "-p"])
        .arg(&file)
        .env("FOCUS_MUSIC_PACKAGE", &p.package)
        .env("FOCUS_MUSIC_BINARY", env!("CARGO_BIN_EXE_focus-music"))
        .env("FOCUS_MUSIC_CONFIG", p.config_home.join("sources.json"))
        .env("FOCUS_MUSIC_LOCAL_CONFIG", &p.local)
        .env("FOCUS_MUSIC_STATE", &p.state)
        .env("FOCUS_MUSIC_RUNTIME", &p.runtime)
        .env("FOCUS_MUSIC_PREFERENCES", &p.preferences);
    let output = util::capture(&mut command, Duration::from_secs(20), None).unwrap();
    let logs = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.success && logs.contains("PASS native UI"), "{logs}");
    assert_eq!(f.controller.state()["volume"].as_f64(), Some(80.));
    assert_eq!(f.controller.preferences()["barDisplay"], "icon");
    assert_eq!(f.controller.preferences()["visualizerWidth"], 120);
    println!("Native UI control, label, volume coalescing, settings, and width checks passed");
}
