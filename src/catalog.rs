use crate::util;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::Path};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    #[serde(default)]
    pub artist: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(skip)]
    pub source_identity: Option<Vec<Value>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Genre {
    pub label: String,
    pub catalog_version: String,
    pub duration: f64,
    pub tracks: Vec<Track>,
}
#[derive(Clone)]
pub struct Catalog {
    pub genres: Vec<(String, Genre)>,
    pub revision: String,
}
impl Catalog {
    pub fn load(config: &Path, local: &Path) -> Result<Self> {
        let bytes = fs::read(config)?;
        let extra = fs::read(local).unwrap_or_default();
        let mut map: serde_json::Map<String, Value> = serde_json::from_slice(&bytes)?;
        if !extra.is_empty() {
            let local: Value = serde_json::from_slice(&extra)?;
            if let Some(entries) = local["genres"].as_object() {
                map.extend(entries.clone());
            }
        }
        let mut revision = bytes;
        revision.push(0);
        revision.extend(&extra);
        let mut genres = Vec::new();
        for (key, value) in map {
            let mut genre: Genre = serde_json::from_value(value.clone())?;
            if let Some(original) = value["tracks"].as_array() {
                for (track, raw) in genre.tracks.iter_mut().zip(original) {
                    let mut identity =
                        vec![raw["url"].clone(), raw["start"].clone(), raw["end"].clone()];
                    if let Some(fingerprint) = raw.get("fingerprint") {
                        identity.push(fingerprint.clone());
                    }
                    track.source_identity = Some(identity);
                }
            }
            if genre.tracks.is_empty() {
                bail!("Genre has no tracks: {key}");
            }
            for track in &genre.tracks {
                crate::library::validate_source(&track.url)?;
                if !track.start.is_finite()
                    || !track.end.is_finite()
                    || !track.duration.is_finite()
                    || track.end <= track.start
                    || track.duration <= 0.
                {
                    bail!("Invalid track boundary: {}", track.title);
                }
            }
            genres.push((key, genre));
        }
        if genres.is_empty() {
            bail!("No music genres");
        }
        Ok(Self {
            genres,
            revision: util::digest(revision),
        })
    }
    pub fn get(&self, key: &str) -> Option<&Genre> {
        self.genres.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn first(&self) -> &str {
        if self.get("lofi-hiphop").is_some() {
            "lofi-hiphop"
        } else {
            &self.genres[0].0
        }
    }
    pub fn list(&self) -> Value {
        json!(self.genres.iter().map(|(key,g)|json!({"value":key,"label":g.label,"track_count":g.tracks.len(),"hours":(g.duration/360.).round()/10.})).collect::<Vec<_>>())
    }
    pub fn audit(&self) -> Result<Vec<String>> {
        let mut identities = std::collections::HashSet::new();
        let mut segments = std::collections::HashSet::new();
        let mut lines = Vec::new();
        for (key, source) in &self.genres {
            let mut seconds = 0.;
            for track in &source.tracks {
                let segment = format!("{}\0{}\0{}", track.url, track.start, track.end);
                if !identities.insert(&track.id) || !segments.insert(segment) {
                    bail!("Duplicate track: {} — {}", track.artist, track.title);
                }
                if track.start < 0. || (track.duration - (track.end - track.start)).abs() > 1e-6 {
                    bail!("Invalid boundary: {}", track.title);
                }
                seconds += track.duration;
            }
            if (seconds - source.duration).abs() > 1e-6 || seconds < 86400. {
                bail!("Invalid or insufficient duration: {key}");
            }
            lines.push(format!(
                "{}: {} tracks, {:0.1} hours",
                source.label,
                source.tracks.len(),
                seconds / 3600.
            ));
        }
        lines.push("No duplicate metadata IDs or exact source segments. Audio fingerprints are not checked.".into());
        Ok(lines)
    }
}
