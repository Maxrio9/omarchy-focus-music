use crate::{
    catalog::{Genre, Track},
    util,
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::HashSet, fs, os::unix::fs::MetadataExt, path::PathBuf, process::Command,
    time::Duration,
};
pub fn local_path(value: &str) -> Option<PathBuf> {
    if let Some(rest) = value.strip_prefix("file:") {
        let path = if let Some(authority) = rest.strip_prefix("//") {
            if let Some(local) = authority.strip_prefix("localhost/") {
                format!("/{local}")
            } else if authority.starts_with('/') {
                authority.to_string()
            } else {
                return None;
            }
        } else {
            rest.to_string()
        };
        let path = path.split(['?', '#']).next()?;
        let decoded = percent_encoding::percent_decode_str(path)
            .decode_utf8()
            .ok()?;
        let path = PathBuf::from(decoded.as_ref());
        return path.is_absolute().then_some(path);
    }
    let path = util::expand(value);
    path.is_absolute().then_some(path)
}
pub fn index(folder: &str) -> Result<(Value, Vec<String>)> {
    let root = fs::canonicalize(util::expand(folder))?;
    if !root.is_dir() {
        bail!("Music folder does not exist");
    }
    let ffprobe = util::executable("ffprobe")
        .context("Indexing local music requires ffprobe (included with FFmpeg)")?;
    let mut directories = fs::read_dir(&root)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect::<Vec<_>>();
    directories.sort_by_key(|p| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });
    let mut genres = serde_json::Map::new();
    let mut skipped = Vec::new();
    let mut seen = HashSet::new();
    let slug = regex::Regex::new("[^a-z0-9]+")?;
    for directory in directories {
        let mut paths = walkdir::WalkDir::new(&directory)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok().map(|e| e.path().to_path_buf()))
            .collect::<Vec<_>>();
        paths.sort();
        let mut tracks = Vec::new();
        for path in paths {
            if !path.is_file() {
                continue;
            }
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if ![
                "mp3", "m4a", "flac", "ogg", "opus", "wav", "aac", "aif", "aiff", "alac", "wma",
                "mka", "mp4", "webm", "ape", "wv",
            ]
            .contains(&extension.as_str())
            {
                continue;
            }
            let resolved = match fs::canonicalize(&path) {
                Ok(p) => p,
                Err(_) => {
                    skipped.push(
                        path.strip_prefix(&root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .into_owned(),
                    );
                    continue;
                }
            };
            if !seen.insert(resolved.clone()) {
                continue;
            }
            let result = (|| -> Result<Track> {
                let output=util::capture(Command::new(&ffprobe).args(["-v","error","-select_streams","a:0","-show_entries","format=duration:format_tags=artist,title,album_artist:stream=duration:stream_tags=artist,title,album_artist","-of","json"]).arg(&resolved),Duration::from_secs(30),None)?;
                if !output.success {
                    bail!("Unreadable music");
                }
                let data: Value = serde_json::from_slice(&output.stdout)?;
                let stream = data["streams"]
                    .as_array()
                    .and_then(|a| a.first())
                    .context("No audio stream")?;
                let duration = data["format"]["duration"]
                    .as_str()
                    .or_else(|| stream["duration"].as_str())
                    .unwrap_or("0")
                    .parse::<f64>()?;
                if !duration.is_finite() || duration <= 0. {
                    bail!("No finite duration");
                }
                let mut tags = serde_json::Map::new();
                for info in [&stream["tags"], &data["format"]["tags"]] {
                    if let Some(map) = info.as_object() {
                        for (k, v) in map {
                            tags.insert(k.to_lowercase(), v.clone());
                        }
                    }
                }
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                let mut artist = util::text(
                    &tags
                        .get("artist")
                        .or_else(|| tags.get("album_artist"))
                        .cloned()
                        .unwrap_or(Value::Null),
                )
                .to_string();
                let mut title = tags
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&stem)
                    .to_string();
                if artist.is_empty()
                    && !tags.contains_key("title")
                    && let Some((a, t)) = stem.split_once(" — ").or_else(|| stem.split_once(" - "))
                {
                    artist = a.into();
                    title = t.into();
                }
                let metadata = fs::metadata(&resolved)?;
                let fingerprint = format!(
                    "{}:{}",
                    metadata.len(),
                    metadata.mtime() as i128 * 1_000_000_000 + metadata.mtime_nsec() as i128
                );
                let resolved_string = resolved.to_string_lossy().into_owned();
                Ok(Track {
                    id: format!("local-{}", util::digest(resolved_string.as_bytes())),
                    artist: if artist.is_empty() {
                        "Unknown artist".into()
                    } else {
                        artist
                    },
                    title,
                    url: resolved_string,
                    start: 0.,
                    end: duration,
                    duration,
                    fingerprint: Some(fingerprint.clone()),
                    source_identity: Some(vec![
                        json!(resolved.to_string_lossy()),
                        json!(0),
                        json!(duration),
                        json!(fingerprint),
                    ]),
                })
            })();
            match result {
                Ok(track) => tracks.push(track),
                Err(_) => skipped.push(
                    path.strip_prefix(&root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned(),
                ),
            }
        }
        if tracks.is_empty() {
            continue;
        }
        let name = directory.file_name().unwrap().to_string_lossy();
        let clean = slug
            .replace_all(&name.to_lowercase(), "-")
            .trim_matches('-')
            .to_string();
        let mut key = format!("local-{}", if clean.is_empty() { "genre" } else { &clean });
        if genres.contains_key(&key) {
            key.push('-');
            key.push_str(&util::digest(name.as_bytes())[..8]);
        }
        let track_values = tracks
            .iter()
            .map(|track| {
                let mut value = serde_json::to_value(track).unwrap();
                value["start"] = json!(0);
                value
            })
            .collect::<Vec<_>>();
        let version = util::digest(util::python_json(&json!(track_values), true, true));
        let genre = Genre {
            label: format!("Local · {name}"),
            catalog_version: version,
            duration: tracks.iter().map(|t| t.duration).sum(),
            tracks,
        };
        let mut value = serde_json::to_value(genre)?;
        value["tracks"] = json!(track_values);
        genres.insert(key, value);
    }
    if genres.is_empty() {
        bail!("No readable music found in genre subfolders");
    }
    Ok((
        json!({"root":root.to_string_lossy(),"genres":genres}),
        skipped,
    ))
}
