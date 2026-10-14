use anyhow::{Context, Result, bail};
use omarchy_focus_music::{
    controller::{Controller, Paths},
    util,
};
use serde_json::json;
use std::process::Command;
fn execute() -> Result<()> {
    util::signal_setup()?;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = args.first().map(String::as_str).unwrap_or("music");
    let paths = Paths::new(util::package_root()?)?;
    if mode == "visualizer" {
        return omarchy_focus_music::visualizer::run(&paths);
    }
    if mode == "compact" {
        match args.get(1).map(String::as_str) {
            Some("--disable") => return omarchy_focus_music::compact::disable(),
            Some("--restore") => return omarchy_focus_music::compact::restore(),
            _ => return omarchy_focus_music::compact::enable(),
        }
    }
    if !["media", "normalize", "audit"].contains(&mode) {
        let commands = if mode == "music" {
            args.get(1..).unwrap_or_default()
        } else {
            &args[..]
        };
        match commands.first().map(String::as_str) {
            Some("settings") => {
                println!(
                    "{}",
                    omarchy_focus_music::preferences::read(&paths.preferences, &paths.shell)
                );
                return Ok(());
            }
            Some("settings-set") => {
                let changes =
                    serde_json::from_str(commands.get(1).context("Missing settings JSON")?)?;
                println!(
                    "{}",
                    omarchy_focus_music::preferences::configure(&paths, &changes)?
                );
                return Ok(());
            }
            _ => {}
        }
    }
    if !["media", "normalize", "audit"].contains(&mode) {
        let commands = if mode == "music" {
            args.get(1..).unwrap_or_default()
        } else {
            &args[..]
        };
        let action = commands.first().map(String::as_str).unwrap_or("menu");
        if let Some(response) = omarchy_focus_music::rpc::request(
            &paths,
            action,
            commands.get(1..).unwrap_or_default(),
        )? {
            if action == "is-playing" {
                if response != true {
                    std::process::exit(1);
                }
            } else if !response.is_null() {
                println!("{response}");
            }
            return Ok(());
        }
    }
    let controller = Controller::new(paths)?;
    if mode == "media" {
        return omarchy_focus_music::media::run(controller);
    }
    if mode == "normalize" {
        let result = omarchy_focus_music::normalizer::run(&controller, &args[1..])
            .unwrap_or_else(|_| json!({"state":"unavailable","gain_db":0}));
        println!("{result}");
        return Ok(());
    }
    if mode == "audit" {
        let catalog = omarchy_focus_music::catalog::Catalog::load(
            &controller.paths.package.join("data/sources.json"),
            &controller.paths.package.join("data/no-local-index"),
        )?;
        for line in catalog.audit()? {
            println!("{line}");
        }
        return Ok(());
    }
    let arguments = if mode == "music" {
        args.get(1..).unwrap_or_default()
    } else {
        &args[..]
    };
    let action = arguments.first().map(String::as_str).unwrap_or("menu");
    let arguments = arguments.get(1..).unwrap_or_default();
    match action {
        "menu" => util::run(Command::new("omarchy-shell").args([
            "shell",
            "toggle",
            "maxrio9.focus-music",
        ]))?,
        "help" | "--help" | "-h" => println!(
            "Usage: focus-music {{menu|genres|play GENRE|toggle|previous|skip|stop|volume DELTA|status|local FOLDER|place left|place right|compact-bar|settings|settings-set JSON}}"
        ),
        "genres" => println!("{}", controller.catalog.list()),
        "status" => println!("{}", controller.status()),
        "settings" => println!("{}", controller.preferences()),
        "settings-set" => println!(
            "{}",
            controller.configure(&serde_json::from_str(
                arguments.first().context("Missing settings JSON")?
            )?)?
        ),
        "place" => {
            let side = arguments.first().context("Missing placement")?;
            if !["left", "right"].contains(&side.as_str()) {
                bail!("Placement must be left or right");
            }
            util::run(Command::new("omarchy").args([
                "bar",
                "move",
                "maxrio9.focus-music",
                "--section",
                "center",
                if side == "left" {
                    "--before"
                } else {
                    "--after"
                },
                "omarchy.clock",
            ]))?;
        }
        "compact-bar" => omarchy_focus_music::compact::enable()?,
        "restart-player" => println!("{}", controller.restart_player()?),
        "local" => {
            let (library, skipped) = omarchy_focus_music::library::index(
                arguments.first().context("Missing music folder")?,
            )?;
            util::atomic_json(&controller.paths.local, &library)?;
            let genres=library["genres"].as_object().unwrap().iter().map(|(key,g)|json!({"value":key,"label":g["label"],"track_count":g["tracks"].as_array().unwrap().len(),"hours":(util::number(&g["duration"],0.)/360.).round()/10.})).collect::<Vec<_>>();
            println!("{}", json!({"genres":genres,"skipped_files":skipped}));
        }
        "is-playing" => {
            let status = controller.status();
            let playing = status["genre"].as_str() == arguments.first().map(String::as_str)
                && ["Playing", "Loading", "Paused"].contains(&util::text(&status["phase"]));
            if !playing {
                std::process::exit(1);
            }
        }
        _ => {
            if let Some(result) = controller.action(action, arguments)? {
                println!("{result}");
            }
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = execute() {
        eprintln!("Focus music: {error:#}");
        if !util::stopping() {
            util::notify(&error.to_string());
        }
        std::process::exit(1);
    }
}
