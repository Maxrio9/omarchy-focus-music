use crate::util;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
pub const RIGHT_ORIGINAL: &str = include_str!("../templates/right_original.qml.txt");
pub const RIGHT_COMPACT: &str = include_str!("../templates/right_compact.qml.txt");
pub const HOVER_ORIGINAL: &str = include_str!("../templates/hover_original.qml.txt");
pub const HOVER_COMPACT: &str = include_str!("../templates/hover_compact.qml.txt");
pub const HOVER_FUNCTION: &str = include_str!("../templates/hover_function.qml.txt");
const SIGNATURE: &str =
    "readonly property bool collapsed: !!bar && bar.centerSectionRevealHeld === true";
pub fn patch(source: &str) -> Result<String> {
    if source.contains("// Focus Music compact bar") {
        return Ok(source.into());
    }
    if source.matches(RIGHT_ORIGINAL).count() != 1
        || (!source.contains(HOVER_FUNCTION) && source.matches(HOVER_ORIGINAL).count() != 2)
    {
        bail!("This Omarchy bar version is not supported by compact-bar");
    }
    let source = source.replace(RIGHT_ORIGINAL, RIGHT_COMPACT);
    if source.contains(HOVER_FUNCTION) {
        return Ok(source);
    }
    Ok(source.replace(HOVER_ORIGINAL, HOVER_COMPACT).replacen(
        "  function setCenterSectionHovered(hovered) {",
        &(HOVER_FUNCTION.to_string() + "  function setCenterSectionHovered(hovered) {"),
        1,
    ))
}
fn state_home() -> PathBuf {
    util::xdg("XDG_STATE_HOME", ".local/state").join("focus-music")
}
fn file_for(directory: &Path, manifest: &Value, key: &str) -> Result<PathBuf> {
    util::checked_relative(directory, util::text(&manifest["entryPoints"][key]))
}
pub fn disable() -> Result<()> {
    let plugins = util::xdg("XDG_CONFIG_HOME", ".config").join("omarchy/plugins");
    if !plugins.is_dir() {
        return Ok(());
    }
    let backup = state_home()
        .join("backups")
        .join(util::backup_name("compact-off"));
    let mut changed = Vec::new();
    for entry in fs::read_dir(plugins)? {
        let directory = entry?.path();
        let Ok(manifest) = util::read_json(&directory.join("manifest.json")) else {
            continue;
        };
        let origin = util::text(&manifest["omarchy"]["clonedFrom"]);
        let (kind, key) = match origin {
            "omarchy.bar" => ("bar", "bar"),
            "omarchy.workspaces" => ("workspaces", "barWidget"),
            _ => continue,
        };
        let Ok(file) = file_for(&directory, &manifest, key) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(&file) else {
            continue;
        };
        let updated = if kind == "bar" && source.contains(RIGHT_COMPACT) {
            source.replace(RIGHT_COMPACT, RIGHT_ORIGINAL)
        } else if kind == "workspaces"
            && source.contains("Keep the numbers on wide desktops")
            && source.contains(SIGNATURE)
        {
            source.replace(SIGNATURE,"readonly property bool collapsed: false && !!bar && bar.centerSectionRevealHeld === true")
        } else {
            continue;
        };
        if updated != source {
            util::private_dir(&backup)?;
            let name = directory.file_name().unwrap().to_string_lossy();
            fs::copy(
                &file,
                backup.join(format!(
                    "{name}-{}",
                    file.file_name().unwrap().to_string_lossy()
                )),
            )?;
            fs::write(file, updated)?;
            changed.push(json!({"directory":name,"kind":kind}));
        }
    }
    if !changed.is_empty() {
        util::atomic_json(&state_home().join("compact-restore.json"), &json!(changed))?;
    }
    Ok(())
}
pub fn restore() -> Result<()> {
    let record = state_home().join("compact-restore.json");
    let Ok(changed) = util::read_json(&record) else {
        return Ok(());
    };
    let backup = state_home()
        .join("backups")
        .join(util::backup_name("compact-restore"));
    for item in changed.as_array().unwrap_or(&Vec::new()) {
        let name = util::text(&item["directory"]);
        if name.is_empty()
            || name == "."
            || name == ".."
            || Path::new(name).file_name().and_then(|s| s.to_str()) != Some(name)
        {
            continue;
        }
        let directory = util::xdg("XDG_CONFIG_HOME", ".config")
            .join("omarchy/plugins")
            .join(name);
        let Ok(manifest) = util::read_json(&directory.join("manifest.json")) else {
            continue;
        };
        let kind = util::text(&item["kind"]);
        let origin = util::text(&manifest["omarchy"]["clonedFrom"]);
        let key = if kind == "bar" && origin == "omarchy.bar" {
            "bar"
        } else if kind == "workspaces" && origin == "omarchy.workspaces" {
            "barWidget"
        } else {
            continue;
        };
        let Ok(file) = file_for(&directory, &manifest, key) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(&file) else {
            continue;
        };
        let updated = if kind == "bar" && source.contains(HOVER_FUNCTION) {
            source.replace(RIGHT_ORIGINAL, RIGHT_COMPACT)
        } else if kind == "workspaces" && source.contains("Keep the numbers on wide desktops") {
            source.replace(
                "readonly property bool collapsed: false && !!bar",
                "readonly property bool collapsed: !!bar",
            )
        } else {
            source.clone()
        };
        if updated != source {
            util::private_dir(&backup)?;
            fs::copy(
                &file,
                backup.join(format!(
                    "{name}-{}",
                    file.file_name().unwrap().to_string_lossy()
                )),
            )?;
            fs::write(file, updated)?;
        }
    }
    fs::remove_file(record)?;
    Ok(())
}
pub fn enable() -> Result<()> {
    let config_home = util::xdg("XDG_CONFIG_HOME", ".config");
    let config = config_home.join("omarchy/shell.json");
    let value = util::read_json(&config)?;
    let current = value["bar"]["id"].as_str().unwrap_or("omarchy.bar");
    if fs::canonicalize(&config_home)? != fs::canonicalize(util::home().join(".config"))? {
        bail!("compact-bar requires Omarchy at the standard user config location");
    }
    let user = env::var("USER")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            util::home()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
    let clone_id = format!("{user}.bar");
    if current != "omarchy.bar" && current != clone_id {
        bail!("A different custom bar is active; compact-bar leaves it unchanged");
    }
    let clone = config_home.join("omarchy/plugins").join(&clone_id);
    let backup = state_home()
        .join("backups")
        .join(util::backup_name("compact-bar"));
    util::private_dir(&backup)?;
    fs::copy(&config, backup.join("shell.json"))?;
    if !clone.exists() {
        let native = util::nonempty("OMARCHY_PATH")
            .unwrap_or_else(|| PathBuf::from("/usr/share/omarchy"))
            .join("shell/plugins/bar/Bar.qml");
        patch(&fs::read_to_string(native)?)?;
        util::run(Command::new("omarchy").args(["plugin", "clone", "omarchy.bar"]))?;
    }
    let file = clone.join("Bar.qml");
    let source = fs::read_to_string(&file)?;
    let updated = patch(&source)?;
    if updated != source {
        fs::copy(&file, backup.join("Bar.qml"))?;
        fs::write(file, updated)?;
    }
    util::run(Command::new("omarchy").args(["bar", "use", &clone_id]))?;
    util::run(Command::new("omarchy").args(["restart", "shell"]))?;
    println!("Compact bar enabled. To restore the standard bar: omarchy bar use omarchy.bar");
    Ok(())
}
