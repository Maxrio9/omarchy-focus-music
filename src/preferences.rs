use crate::util;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::path::Path;
pub fn defaults() -> Value {
    json!({"barDisplay":"banner","placement":"left","bannerWidth":null,"visualizerWidth":100,"otherMedia":"pause-resume"})
}
pub fn validate(value: &Value) -> Result<()> {
    let map = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Settings must be an object"))?;
    for (key, value) in map {
        match key.as_str() {
            "barDisplay" => {
                if !["banner", "visualizer", "icon"].contains(&util::text(value)) {
                    bail!("Invalid barDisplay");
                }
            }
            "placement" => {
                if !["left", "clock-right", "right"].contains(&util::text(value)) {
                    bail!("Invalid placement");
                }
            }
            "otherMedia" => {
                if !["pause-resume", "duck-strong", "duck-gentle", "none"]
                    .contains(&util::text(value))
                {
                    bail!("Invalid otherMedia");
                }
            }
            "bannerWidth" | "visualizerWidth" => {
                if !(key == "bannerWidth" && value.is_null())
                    && !value.as_u64().is_some_and(|v| (80..=500).contains(&v))
                {
                    bail!("Width must be between 80 and 500");
                }
            }
            _ => bail!("Unknown display setting: {key}"),
        }
    }
    Ok(())
}
pub fn read(path: &Path, shell: &Path) -> Value {
    let mut settings = defaults();
    if let Ok(mut saved) = util::read_json(path)
        && let Some(map) = saved.as_object_mut()
    {
        map.remove("popupVisualizer");
        map.remove("volumeReactive");
        if validate(&saved).is_ok() {
            settings
                .as_object_mut()
                .unwrap()
                .extend(saved.as_object().unwrap().clone());
            return settings;
        }
    }
    if let Ok(shell) = util::read_json(shell)
        && let Some(layout) = shell["bar"]["layout"].as_object()
    {
        for (region, entries) in layout {
            if let Some(entries) = entries.as_array() {
                for (index, entry) in entries.iter().enumerate() {
                    if entry["id"] != "maxrio9.focus-music" {
                        continue;
                    }
                    if region == "right" {
                        settings["placement"] = json!("right");
                        settings["barDisplay"] = json!("icon");
                    } else if region == "center" {
                        let clock = entries
                            .iter()
                            .position(|e| e["id"] == "omarchy.clock")
                            .unwrap_or(index + 1);
                        settings["placement"] =
                            json!(if index < clock { "left" } else { "clock-right" });
                    }
                    if let Some(width) = entry.get("bannerWidth")
                        && width.as_u64().is_some_and(|w| (80..=500).contains(&w))
                    {
                        settings["bannerWidth"] = width.clone();
                    }
                }
            }
        }
    }
    settings
}

pub fn configure(paths: &crate::controller::Paths, changes: &Value) -> Result<Value> {
    validate(changes)?;
    let before = read(&paths.preferences, &paths.shell);
    let updated = {
        let _lock = util::Lock::acquire(&paths.runtime.join("preferences.lock"), false)?;
        let mut updated = read(&paths.preferences, &paths.shell);
        updated
            .as_object_mut()
            .unwrap()
            .extend(changes.as_object().unwrap().clone());
        if changes["placement"] == "right" {
            updated["barDisplay"] = json!("icon");
        }
        util::atomic_json(&paths.preferences, &updated)?;
        updated
    };
    if let Some(placement) = changes.get("placement")
        && placement != &before["placement"]
    {
        if placement == "right" {
            crate::compact::disable()?;
        } else if before["placement"] == "right" {
            crate::compact::restore()?;
        }
        let mut command = std::process::Command::new("omarchy");
        command.args(["bar", "move", "maxrio9.focus-music"]);
        if placement == "right" {
            command.args(["--section", "right", "--index", "0"]);
        } else {
            command.args([
                "--section",
                "center",
                if placement == "left" {
                    "--before"
                } else {
                    "--after"
                },
                "omarchy.clock",
            ]);
        }
        util::run(&mut command)?;
    }
    Ok(updated)
}
