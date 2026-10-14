mod common;
use common::Fixture;
use omarchy_focus_music::rpc;
use serde_json::json;
use std::sync::{Arc, Mutex};
#[test]
fn cached_commands_preserve_context_and_never_retry_partial_actions() {
    let f = Fixture::new();
    f.controller.play("piano").unwrap();
    let paths = f.controller.paths.clone();
    let shared = Arc::new(Mutex::new(f.controller.clone()));
    let server = rpc::Server::start(shared.clone()).unwrap();
    let status = rpc::request(&paths, "status", &[]).unwrap().unwrap();
    assert_eq!(status["phase"], "Playing");
    let response = rpc::request(&paths, "volume-set", &["37".into()])
        .unwrap()
        .unwrap();
    assert_eq!(response["volume"], 37.);
    assert_eq!(f.controller.prop("volume").unwrap().as_f64(), Some(37.));
    let genres = rpc::request(&paths, "genres", &[]).unwrap().unwrap();
    assert_eq!(genres.as_array().unwrap().len(), 2);
    let mut different = paths.clone();
    different.state = paths.runtime.join("other-state.json");
    assert!(
        rpc::request(&different, "volume-set", &["90".into()])
            .unwrap()
            .is_none()
    );
    assert_eq!(f.controller.state()["volume"].as_f64(), Some(37.));
    assert!(rpc::request(&paths, "volume-set", &["NaN".into()]).is_err());
    assert_eq!(f.controller.state()["volume"].as_f64(), Some(37.));
    rpc::request(&paths, "skip", &["keep-pause".into()]).unwrap();
    assert_eq!(f.controller.prop("playlist-pos").unwrap(), json!(1));
    drop(server);
    assert!(rpc::request(&paths, "status", &[]).unwrap().is_none());
    assert!(!paths.runtime.join("controller.sock").exists());
}
