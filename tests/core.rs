use omarchy_focus_music::{
    catalog::Catalog,
    compact,
    controller::Controller,
    ipc, library, normalizer, preferences,
    spectrum::{RATE, SIZE, Spectrum, bar_count},
    util, visualizer,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};
mod common;
use common::Fixture;
#[test]
fn catalog_is_complete_and_unique() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let catalog =
        Catalog::load(&root.join("data/sources.json"), &root.join("data/missing")).unwrap();
    assert_eq!(catalog.genres.len(), 6);
    assert_eq!(catalog.audit().unwrap().len(), 7);
    let mut names = std::collections::HashSet::new();
    for (_, genre) in &catalog.genres {
        for track in &genre.tracks {
            assert!(names.insert((&track.artist, &track.title)));
        }
    }
}
#[test]
fn shuffle_consumes_played_entries_and_never_resumes() {
    let f = Fixture::new();
    let p = &f.controller;
    p.play("lofi-hiphop").unwrap();
    let order = p.state()["active_order"].as_array().unwrap().clone();
    let track = &p.catalog.get("lofi-hiphop").unwrap().tracks[order[1].as_u64().unwrap() as usize];
    f.property("playlist-pos", json!(1));
    f.property("path", json!(track.url));
    f.property("time-pos", json!(999));
    p.play("piano").unwrap();
    p.play("lofi-hiphop").unwrap();
    let remaining = p.state()["active_order"].as_array().unwrap().clone();
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().all(|v| order[2..].contains(v)));
    let current = p.status();
    let track = &p.catalog.get("lofi-hiphop").unwrap().tracks
        [current["track"].as_u64().unwrap() as usize - 1];
    assert_eq!(p.prop("time-pos").unwrap().as_f64().unwrap(), track.start);
}
#[test]
fn previous_and_pause_preserve_boundaries_and_intent() {
    let f = Fixture::new();
    let p = &f.controller;
    p.play("piano").unwrap();
    p.action("skip", &[]).unwrap();
    p.action("previous", &[]).unwrap();
    assert_eq!(p.prop("playlist-pos").unwrap(), 0);
    p.action("previous", &[]).unwrap();
    let saved = p.state();
    assert_eq!(
        p.prop("time-pos").unwrap().as_f64().unwrap(),
        p.selected_track(&saved, 0).unwrap().start
    );
    for request in ["1", "1", "0"] {
        let status = p.action("pause-set", &[request.into()]).unwrap().unwrap();
        assert_eq!(
            status["phase"],
            if request == "1" { "Paused" } else { "Playing" }
        );
    }
    p.action("pause-set", &["1".into()]).unwrap();
    p.action("skip", &["keep-pause".into()]).unwrap();
    assert_eq!(p.status()["phase"], "Paused");
    assert!(
        f.mock
            .commands
            .lock()
            .unwrap()
            .iter()
            .any(|args| args[0] == "seek")
    );
}
#[test]
fn track_loading_keeps_metadata_and_pause_label() {
    let f = Fixture::new();
    let p = &f.controller;
    p.play("piano").unwrap();
    p.action("skip", &[]).unwrap();
    p.action("skip", &[]).unwrap();
    let expected = p.selected_track(&p.state(), 2).unwrap().id.clone();
    f.property("time-pos", Value::Null);
    for (idle, paused) in [(false, false), (false, true), (true, false), (true, true)] {
        f.property("idle-active", json!(idle));
        f.property("pause", json!(paused));
        let status = p.status();
        assert_eq!(status["track_id"], expected);
        assert_eq!(status["phase"], if paused { "Paused" } else { "Loading" });
    }
}
#[test]
fn interruptions_only_resume_auto_pauses() {
    let f = Fixture::new();
    let p = &f.controller;
    p.play("piano").unwrap();
    p.sync_other_media(true).unwrap();
    assert_eq!(p.status()["phase"], "Paused");
    assert_eq!(p.state()["auto_paused"], true);
    p.sync_other_media(false).unwrap();
    assert_eq!(p.status()["phase"], "Playing");
    p.sync_other_media(true).unwrap();
    p.action("pause-set", &["1".into()]).unwrap();
    p.sync_other_media(false).unwrap();
    assert_eq!(p.status()["phase"], "Paused");
    p.sync_other_media(true).unwrap();
    p.sync_other_media(false).unwrap();
    assert_eq!(p.status()["phase"], "Paused");
}
#[test]
fn volume_reduction_preserves_base_and_scroll_changes() {
    let f = Fixture::new();
    let p = &f.controller;
    p.play("piano").unwrap();
    p.action("volume-set", &["40".into()]).unwrap();
    for (mode, factor) in [("duck-strong", 0.4), ("duck-gentle", 0.75), ("none", 1.)] {
        p.configure(&json!({"otherMedia":mode})).unwrap();
        p.sync_other_media(true).unwrap();
        assert_eq!(p.prop("volume").unwrap().as_f64().unwrap(), 40. * factor);
        assert_eq!(p.status()["volume"], 40.);
        p.action("volume", &["20".into()]).unwrap();
        assert_eq!(p.prop("volume").unwrap().as_f64().unwrap(), 60. * factor);
        p.sync_other_media(false).unwrap();
        assert_eq!(p.prop("volume").unwrap().as_f64().unwrap(), 60.);
        p.action("volume-set", &["40".into()]).unwrap();
    }
    p.configure(&json!({"otherMedia":"pause-resume"})).unwrap();
    p.sync_other_media(true).unwrap();
    p.configure(&json!({"otherMedia":"duck-gentle"})).unwrap();
    p.sync_other_media(true).unwrap();
    assert_eq!(p.status()["phase"], "Playing");
}
#[test]
fn settings_migrate_and_reject_invalid_values() {
    let f = Fixture::new();
    let p = &f.controller;
    util::atomic_json(&p.paths.preferences,&json!({"barDisplay":"visualizer","popupVisualizer":false,"volumeReactive":false,"bannerWidth":240})).unwrap();
    assert_eq!(p.preferences()["barDisplay"], "visualizer");
    assert!(p.preferences().get("popupVisualizer").is_none());
    p.configure(&json!({"visualizerWidth":120,"otherMedia":"none"}))
        .unwrap();
    let before = fs::read(&p.paths.preferences).unwrap();
    for bad in [
        json!({"barDisplay":"fake"}),
        json!({"visualizerWidth":null}),
        json!({"otherMedia":"fake"}),
        json!({"bannerWidth":10000}),
        json!({"volumeReactive":true}),
    ] {
        assert!(p.configure(&bad).is_err());
        assert_eq!(fs::read(&p.paths.preferences).unwrap(), before);
    }
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(&p.paths.preferences)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
#[test]
fn infers_native_bar_layout() {
    let root = tempfile::tempdir().unwrap();
    let shell = root.path().join("shell.json");
    util::atomic_json(
        &shell,
        &json!({"bar":{"layout":{"right":[{"id":"maxrio9.focus-music"}]}}}),
    )
    .unwrap();
    let settings = preferences::read(&root.path().join("missing"), &shell);
    assert_eq!(settings["placement"], "right");
    assert_eq!(settings["barDisplay"], "icon");
}
#[test]
fn ipc_ignores_events_and_reports_property_errors() {
    let f = Fixture::new();
    assert_eq!(
        ipc::property(&f.controller.paths.socket, "volume").unwrap(),
        20
    );
    assert!(ipc::property(&f.controller.paths.socket, "missing-property").is_err());
    let mut client = ipc::Ipc::connect(&f.controller.paths.socket).unwrap();
    assert_eq!(client.property("volume").unwrap(), 20);
    assert_eq!(client.property("pause").unwrap(), false);
}
#[test]
fn frequency_and_volume_match_with_fixed_and_variable_counts() {
    assert_eq!(bar_count(120), 6);
    assert_eq!(bar_count(500), 30);
    let mut spectrum = Spectrum::default();
    assert_eq!(
        spectrum.analyze(&[0.; SIZE], 100., &[5, 6]),
        vec![vec![0.; 5], vec![0.; 6]]
    );
    for (index, frequency) in [100., 300., 1000., 3000., 7000.].iter().enumerate() {
        let samples = (0..SIZE)
            .map(|i| 0.15 * (2. * std::f64::consts::PI * frequency * i as f64 / RATE as f64).sin())
            .collect::<Vec<_>>();
        let loud = spectrum.analyze(&samples, 100., &[5, 6, 14, 30]);
        assert_eq!(
            loud[0]
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0,
            index
        );
        let quiet = spectrum.analyze(
            &samples.iter().map(|s| s * 0.008).collect::<Vec<_>>(),
            20.,
            &[5, 6, 14, 30],
        );
        for (a, b) in loud.iter().flatten().zip(quiet.iter().flatten()) {
            assert!((a - b).abs() < 1e-8);
        }
    }
}
#[test]
fn never_selects_microphone_or_another_app() {
    let objects = json!([{"id":1,"type":"PipeWire:Interface:Client","info":{"props":{"application.process.id":"123"}}},{"id":2,"type":"PipeWire:Interface:Node","info":{"props":{"client.id":1,"media.class":"Audio/Source","object.serial":200}}},{"id":3,"type":"PipeWire:Interface:Node","info":{"props":{"client.id":99,"media.class":"Stream/Output/Audio","object.serial":300}}},{"id":4,"type":"PipeWire:Interface:Node","info":{"props":{"client.id":1,"media.class":"Stream/Output/Audio","object.serial":400}}}]);
    assert_eq!(
        visualizer::player_node(objects.as_array().unwrap(), 123),
        Some(400)
    );
    assert_eq!(
        visualizer::player_node(objects.as_array().unwrap(), 456),
        None
    );
    let mut missing_serial = objects.clone();
    missing_serial[3]["info"]["props"]
        .as_object_mut()
        .unwrap()
        .remove("object.serial");
    assert_eq!(
        visualizer::player_node(missing_serial.as_array().unwrap(), 123),
        None
    );
    missing_serial[3]["info"]["props"]["object.serial"] = json!(0);
    assert_eq!(
        visualizer::player_node(missing_serial.as_array().unwrap(), 123),
        None
    );
}
#[test]
fn compact_patch_is_scoped_and_idempotent() {
    let original = format!(
        "{}\n{}\n{}\n  function setCenterSectionHovered(hovered) {{",
        compact::RIGHT_ORIGINAL,
        compact::HOVER_ORIGINAL,
        compact::HOVER_ORIGINAL
    );
    let patched = compact::patch(&original).unwrap();
    assert!(patched.contains("opacity: compactHidden ? 0 : 1"));
    assert!(!patched.contains("visible: !compactHidden"));
    assert!(patched.contains("slot.region !== \"center\""));
    assert_eq!(compact::patch(&patched).unwrap(), patched);
    assert!(compact::patch("unrelated custom bar").is_err());
}
#[test]
fn path_decoder_rejects_remote_file_hosts() {
    assert_eq!(
        library::local_path("file:///tmp/Local%20%C3%BC.wav"),
        Some(PathBuf::from("/tmp/Local ü.wav"))
    );
    assert_eq!(library::local_path("https://example.test/music"), None);
    assert_eq!(
        library::local_path("file://another-machine/tmp/audio"),
        None
    );
}
#[test]
fn gain_limits_peaks_without_compressing_dynamics() {
    assert_eq!(normalizer::gain_for(-18., -3.), -5.);
    assert_eq!(normalizer::gain_for(-30., -12.), 7.);
    assert_eq!(normalizer::gain_for(-40., -1.), -1.);
    assert_eq!(normalizer::gain_for(f64::NEG_INFINITY, -30.), 0.);
    assert_eq!(normalizer::gain_for(-80., -70.), 0.);
}
fn wav(path: &Path, scale: f64, seconds: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let rate = 44100usize;
    let mut data = Vec::new();
    for i in 0..rate * seconds {
        let amplitude = if i < rate * seconds / 2 { 0.02 } else { 0.12 };
        let sample = (32767.
            * amplitude
            * scale
            * (2. * std::f64::consts::PI * 440. * i as f64 / rate as f64).sin())
            as i16;
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
fn pcm(path: &Path) -> Vec<i16> {
    let mut data = Vec::new();
    fs::File::open(path)
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    let mut offset = 12;
    while offset + 8 < data.len() {
        let size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if &data[offset..offset + 4] == b"data" {
            return data[offset + 8..offset + 8 + size]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes(*b))
                .collect();
        }
        offset += 8 + size + (size % 2);
    }
    panic!("No PCM data")
}
#[test]
fn real_ffmpeg_loudness_preserves_quiet_loud_contrast() {
    if util::executable("ffmpeg").is_none() {
        eprintln!("FFmpeg unavailable; waveform test skipped");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let mut levels = Vec::new();
    for (name, scale) in [("quiet", 0.4), ("loud", 1.)] {
        let source = root.path().join(format!("{name}.wav"));
        let output = root.path().join(format!("{name}-normalized.wav"));
        wav(&source, scale, 8);
        let measured = normalizer::measure_source(source.to_str().unwrap(), 0., 8., None).unwrap();
        let gain = normalizer::gain_for(
            util::number(&measured["loudness_lufs"], 0.),
            util::number(&measured["true_peak_dbfs"], 0.),
        );
        util::run(Command::new("ffmpeg").args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            source.to_str().unwrap(),
            "-af",
            &format!("volume={gain}dB"),
            output.to_str().unwrap(),
        ]))
        .unwrap();
        let measured = normalizer::measure_source(output.to_str().unwrap(), 0., 8., None).unwrap();
        levels.push(util::number(&measured["loudness_lufs"], 0.));
        assert!(util::number(&measured["true_peak_dbfs"], 0.) <= -1.9);
        let values = pcm(&output);
        let half = values.len() / 2;
        let rms = |slice: &[i16]| {
            (slice.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / slice.len() as f64).sqrt()
        };
        assert!((rms(&values[half..]) / rms(&values[..half]) - 6.).abs() < 0.02);
    }
    assert!((levels[0] - levels[1]).abs() < 0.2);
    assert!((levels[0] + 23.).abs() < 0.2);
}
#[test]
fn local_index_tags_symlink_dedup_and_offline_cache() {
    if util::executable("ffprobe").is_none() || util::executable("ffmpeg").is_none() {
        return;
    }
    let f = Fixture::new();
    let root = f._root.path().join("Music library ü");
    let source = root.join("Piano/Artist - Title.wav");
    wav(&source, 0.4, 2);
    let tagged = root.join("Jazz/Album/Tagged.flac");
    fs::create_dir_all(tagged.parent().unwrap()).unwrap();
    util::run(Command::new("ffmpeg").args([
        "-nostdin",
        "-v",
        "error",
        "-i",
        source.to_str().unwrap(),
        "-metadata",
        "artist=Tagged Artist",
        "-metadata",
        "title=Tagged Title",
        tagged.to_str().unwrap(),
    ]))
    .unwrap();
    std::os::unix::fs::symlink(&source, root.join("Piano/Duplicate.wav")).unwrap();
    fs::write(root.join("Piano/Broken.mp3"), "not audio").unwrap();
    let (library, skipped) = library::index(root.to_str().unwrap()).unwrap();
    assert_eq!(library["genres"].as_object().unwrap().len(), 2);
    assert_eq!(
        library["genres"]["local-piano"]["tracks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        library["genres"]["local-piano"]["tracks"][0]["title"],
        "Title"
    );
    assert_eq!(
        library["genres"]["local-jazz"]["tracks"][0]["artist"],
        "Tagged Artist"
    );
    assert_eq!(skipped, vec!["Piano/Broken.mp3"]);
    util::atomic_json(&f.controller.paths.local, &library).unwrap();
    let p = Controller::new(f.controller.paths.clone()).unwrap();
    assert_ne!(p.catalog.revision, f.controller.catalog.revision);
    let track = &p.catalog.get("local-piano").unwrap().tracks[0];
    assert_eq!(track.source_identity.as_ref().unwrap()[1], json!(0));
    let first = normalizer::measurement(&p, track, None).unwrap();
    let second = normalizer::measurement(&p, track, None).unwrap();
    assert_eq!(first, second);
    let cache = fs::read_to_string(
        p.paths
            .cache
            .join(format!("{}.json", normalizer::cache_key(track))),
    )
    .unwrap();
    assert!(!cache.contains("http"));
    let mut changed = track.clone();
    changed.source_identity = None;
    changed.fingerprint = Some("changed".into());
    assert_ne!(
        normalizer::cache_key(track),
        normalizer::cache_key(&changed)
    );
}

#[test]
fn legacy_cache_identity_and_ascii_serialization_are_preserved() {
    let fixture = Fixture::new();
    let controller = &fixture.controller;
    let mut track = controller.catalog.get("piano").unwrap().tracks[0].clone();
    track.source_identity = Some(vec![
        json!("https://www.youtube.com/watch?v=abcdefghijk"),
        json!(10),
        json!(20),
    ]);
    assert_eq!(
        normalizer::cache_key(&track),
        "dc18b2165dbe81630f1c9085d1ad6b5b99417070ae6273856fea298dfb5bed19"
    );
    assert_eq!(
        util::python_json(&json!({"path":"ü/😀","volume":2.0}), true, true),
        r#"{"path": "\u00fc/\ud83d\ude00", "volume": 2.0}"#
    );
    let cached = json!({"schema":1,"loudness_lufs":-24.0,"true_peak_dbfs":-7.0});
    util::atomic_json(
        &controller
            .paths
            .cache
            .join(format!("{}.json", normalizer::cache_key(&track))),
        &cached,
    )
    .unwrap();
    assert_eq!(normalizer::cached(controller, &track), Some(cached));
}

#[test]
fn helper_timeout_and_cancellation_do_not_leave_children() {
    let start = std::time::Instant::now();
    let result = util::capture(
        Command::new("sh").args(["-c", "sleep 30"]),
        std::time::Duration::from_millis(100),
        None,
    );
    assert!(result.is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
    let result = util::capture(
        Command::new("sh").args(["-c", "sleep 30"]),
        std::time::Duration::from_secs(30),
        Some(&|| true),
    );
    assert!(result.is_err());
}

#[test]
fn persistent_backend_reaps_finished_detached_children() {
    let child = std::process::Command::new("sh")
        .args(["-c", "exit 0"])
        .spawn()
        .unwrap();
    let path = std::path::PathBuf::from(format!("/proc/{}", child.id()));
    omarchy_focus_music::util::reap(child);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while path.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!path.exists(), "Finished helper was not reaped");
}

#[test]
fn anime_catalog_excludes_looped_tails_and_keeps_artist_credits() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let catalog =
        Catalog::load(&root.join("data/sources.json"), &root.join("data/missing")).unwrap();
    let genre = catalog.get("anime").unwrap();
    assert!(genre.duration >= 86400.);
    let mut seen = std::collections::HashSet::new();
    for track in &genre.tracks {
        assert!(!track.artist.is_empty());
        assert!(seen.insert((&track.url, track.start.to_bits(), track.end.to_bits())));
        assert!(!track.title.to_lowercase().contains("2nd round"));
        if track.url.ends_with("7bP5YwhH9tE") {
            assert!(track.end <= 3820.);
        }
        if track.url.ends_with("cXjDu_1g7Co") {
            assert!(track.end <= 2571.);
        }
        assert!(track.duration <= 1500.);
    }
}
