use crate::{catalog::Track, controller::Controller, library, util};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    process::Command,
    time::{Duration, Instant},
};
pub fn gain_for(loudness: f64, peak: f64) -> f64 {
    if !loudness.is_finite() || !peak.is_finite() || loudness < -60. {
        return 0.;
    }
    (((-23. - loudness).clamp(-30., 12.)).min(-2. - peak) * 100.).round() / 100.
}
pub fn parse_measurement(log: &str) -> Result<Value> {
    let summary = log.rsplit("Summary:").next().unwrap_or(log);
    let integrated = regex::Regex::new(r"\bI:\s*(-?[\d.]+)\s*LUFS")?;
    let peak = regex::Regex::new(r"(?s)True peak:\s*Peak:\s*(-?[\d.]+)\s*dBFS")?;
    let loudness = integrated
        .captures(summary)
        .context("Could not measure track loudness")?[1]
        .parse::<f64>()?;
    let peak = peak
        .captures(summary)
        .context("Could not measure track peak")?[1]
        .parse::<f64>()?;
    if !loudness.is_finite() || !peak.is_finite() {
        bail!("Track has no measurable audio");
    }
    Ok(json!({"loudness_lufs":loudness,"true_peak_dbfs":peak}))
}
pub fn measure_source(
    audio: &str,
    start: f64,
    duration: f64,
    cancel: Option<&dyn Fn() -> bool>,
) -> Result<Value> {
    library::validate_source(audio)?;
    let protocols = if let Some(path) = library::local_path(audio) {
        library::regular_file(&path)?;
        "file"
    } else {
        "http,https,tcp,tls,crypto"
    };
    let last = Cell::new(Instant::now() - Duration::from_secs(3));
    let cancelled = || {
        if last.get().elapsed() < Duration::from_secs(3) {
            false
        } else {
            last.set(Instant::now());
            cancel.is_some_and(|f| f())
        }
    };
    let result = util::capture(
        Command::new("ffmpeg").args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "info",
            "-threads",
            "1",
            "-protocol_whitelist",
            protocols,
            "-ss",
            &start.to_string(),
            "-i",
            audio,
            "-t",
            &duration.to_string(),
            "-vn",
            "-sn",
            "-dn",
            "-af",
            "ebur128=peak=true:framelog=quiet:dualmono=true",
            "-f",
            "null",
            "-",
        ]),
        Duration::from_secs_f64(240f64.max(duration * 2. + 60.)),
        Some(&cancelled),
    )?;
    if !result.success {
        bail!("Track loudness analysis failed");
    }
    parse_measurement(&String::from_utf8_lossy(&result.stderr))
}
pub fn cache_key(track: &Track) -> String {
    let identity = track.source_identity.clone().unwrap_or_else(|| {
        let mut values = vec![json!(track.url), json!(track.start), json!(track.end)];
        if let Some(fingerprint) = &track.fingerprint {
            values.push(json!(fingerprint));
        }
        values
    });
    util::digest(util::python_json(&json!(identity), false, false))
}
pub fn cached(controller: &Controller, track: &Track) -> Option<Value> {
    let value = util::read_json(
        &controller
            .paths
            .cache
            .join(format!("{}.json", cache_key(track))),
    )
    .ok()?;
    if value["schema"] != 1
        || !value["loudness_lufs"].as_f64().is_some_and(f64::is_finite)
        || !value["true_peak_dbfs"].as_f64().is_some_and(f64::is_finite)
    {
        None
    } else {
        Some(value)
    }
}
pub fn measurement(
    controller: &Controller,
    track: &Track,
    cancel: Option<&dyn Fn() -> bool>,
) -> Result<Value> {
    if let Some(cached) = cached(controller, track) {
        return Ok(cached);
    }
    let audio = if let Some(path) = library::local_path(&track.url) {
        path.to_string_lossy().into_owned()
    } else {
        let result = util::capture(
            Command::new("yt-dlp").args([
                "--no-warnings",
                "--no-playlist",
                "-f",
                "bestaudio",
                "-g",
                "--",
                &track.url,
            ]),
            Duration::from_secs(90),
            cancel,
        )?;
        if !result.success {
            bail!("Could not resolve audio for loudness analysis");
        }
        let resolved = String::from_utf8(result.stdout)?
            .lines()
            .next()
            .filter(|s| !s.is_empty())
            .context("Could not resolve audio for loudness analysis")?
            .to_string();
        library::validate_source(&resolved)?;
        if library::local_path(&resolved).is_some() {
            bail!("Online audio must resolve to an HTTP(S) URL");
        }
        resolved
    };
    let mut result = measure_source(&audio, track.start, track.duration, cancel)?;
    result["schema"] = json!(1);
    result["measured_at"] = json!(util::seconds() as u64);
    util::private_dir(&controller.paths.cache)?;
    util::atomic_json(
        &controller
            .paths
            .cache
            .join(format!("{}.json", cache_key(track))),
        &result,
    )?;
    Ok(result)
}
pub fn superseded(controller: &Controller, index: usize, generation: u64, prefetch: bool) -> bool {
    if controller.state()["generation"].as_u64() != Some(generation) {
        return true;
    }
    match controller
        .prop("playlist-pos")
        .ok()
        .and_then(|v| v.as_u64())
    {
        None => true,
        Some(current) => {
            if prefetch {
                current > index as u64
            } else {
                current != index as u64
            }
        }
    }
}
fn ready(value: &Value) -> Value {
    json!({"state":"ready","gain_db":gain_for(util::number(&value["loudness_lufs"],f64::NAN),util::number(&value["true_peak_dbfs"],f64::NAN))})
}
pub fn run(controller: &Controller, args: &[String]) -> Result<Value> {
    if std::env::var("FOCUS_MUSIC_NORMALIZE").ok().as_deref() == Some("0") {
        return Ok(json!({"state":"off","gain_db":0}));
    }
    if util::executable("ffmpeg").is_none() {
        return Ok(json!({"state":"unavailable","gain_db":0}));
    }
    unsafe {
        libc::nice(10);
    }
    let mode = args.first().map(String::as_str).unwrap_or("current");
    let index = args
        .get(1)
        .context("Missing track index")?
        .parse::<usize>()?;
    let generation = args.get(2).context("Missing generation")?.parse::<u64>()?;
    if controller.state()["generation"].as_u64() != Some(generation) {
        return Ok(json!({"state":"superseded","gain_db":0}));
    }
    if mode != "prefetch"
        && let Some(value) = cached(
            controller,
            controller.selected_track(&controller.state(), index)?,
        )
    {
        return Ok(ready(&value));
    }
    let Ok(_lock) = util::Lock::acquire(&controller.paths.runtime.join("normalize.lock"), true)
    else {
        return Ok(json!({"state":"pending","gain_db":0}));
    };
    if mode == "prefetch" {
        for offset in [1, 2] {
            if controller.state()["generation"].as_u64() != Some(generation) || util::stopping() {
                break;
            }
            if let Ok(track) = controller.selected_track(&controller.state(), index + offset) {
                let _ = measurement(
                    controller,
                    track,
                    Some(&|| superseded(controller, index + offset, generation, true)),
                );
            }
        }
        Ok(json!({"state":"prefetched","gain_db":0}))
    } else {
        let result = measurement(
            controller,
            controller.selected_track(&controller.state(), index)?,
            Some(&|| superseded(controller, index, generation, false)),
        )?;
        Ok(ready(&result))
    }
}
