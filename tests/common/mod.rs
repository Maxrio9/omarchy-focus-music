#![allow(dead_code, unused_imports)]
use omarchy_focus_music::{
    catalog::Catalog,
    compact,
    controller::{Controller, Paths},
    ipc, library, normalizer, preferences,
    spectrum::{RATE, SIZE, Spectrum, bar_count},
    util, visualizer,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
pub struct Mock {
    pub props: Arc<Mutex<Value>>,
    pub commands: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Mock {
    fn new(path: &Path) -> Self {
        let listener = UnixListener::bind(path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let props = Arc::new(Mutex::new(
            json!({"idle-active":true,"volume":20,"playlist-pos":null,"pause":false,"path":"","time-pos":null,"user-data/focus-music-normalization":{"state":"ready","gain_db":0}}),
        ));
        let commands = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (p, c, s) = (props.clone(), commands.clone(), stop.clone());
        let playlist = Arc::new(Mutex::new(Vec::<Value>::new()));
        let handle = thread::spawn(move || {
            let mut handles = Vec::new();
            while !s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_millis(100)))
                            .unwrap();
                        let (p, c, s, list) = (p.clone(), c.clone(), s.clone(), playlist.clone());
                        handles.push(thread::spawn(move||{let mut reader=BufReader::new(stream.try_clone().unwrap());while !s.load(Ordering::Relaxed){let mut line=String::new();match reader.read_line(&mut line){Ok(0)=>break,Err(e)if [std::io::ErrorKind::WouldBlock,std::io::ErrorKind::TimedOut].contains(&e.kind())=>continue,Err(_)=>break,_=>()};let Ok(request)=serde_json::from_str::<Value>(&line)else{break;};let args=&request["command"];c.lock().unwrap().push(args.clone());let mut props=p.lock().unwrap();let mut error="success";let data=match args[0].as_str().unwrap_or(""){"get_property"=>match props.get(args[1].as_str().unwrap_or("")){Some(v)=>v.clone(),None=>{error="property unavailable";Value::Null}},"set_property"=>{props[args[1].as_str().unwrap()]=args[2].clone();Value::Null},"cycle"=>{props["pause"]=json!(!props["pause"].as_bool().unwrap());Value::Null},"loadfile"=>{let mut list=list.lock().unwrap();if args[2]=="replace"{list.clear();props["playlist-pos"]=json!(0);props["path"]=args[1].clone();props["time-pos"]=json!(args[4]["start"].as_str().unwrap().parse::<f64>().unwrap());props["idle-active"]=json!(false);}list.push(args.clone());Value::Null},"playlist-next"|"playlist-prev"=>{let list=list.lock().unwrap();let current=props["playlist-pos"].as_u64().unwrap_or(0) as usize;let next=if args[0]=="playlist-next"{(current+1)%list.len()}else{current.saturating_sub(1)};props["playlist-pos"]=json!(next);props["path"]=list[next][1].clone();props["time-pos"]=json!(list[next][4]["start"].as_str().unwrap().parse::<f64>().unwrap());Value::Null},"seek"=>{props["time-pos"]=args[1].clone();Value::Null},"quit"=>{props["idle-active"]=json!(true);props["playlist-pos"]=Value::Null;Value::Null},_=>Value::Null};drop(props);let messages=[json!({"event":"property-change","data":999}),json!({"request_id":99999,"error":"success","data":-1}),json!({"request_id":request["request_id"],"error":error,"data":data})];for response in messages{if writeln!(stream,"{response}").is_err(){return;}}}}));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
            for handle in handles {
                let _ = handle.join();
            }
        });
        Self {
            props,
            commands,
            stop,
            thread: Some(handle),
        }
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.thread.take().unwrap().join();
    }
}
pub struct Fixture {
    pub _root: tempfile::TempDir,
    pub controller: Controller,
    pub mock: Mock,
}
impl Fixture {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = root.path();
        let runtime = base.join("runtime");
        util::private_dir(&runtime).unwrap();
        let paths = Paths {
            package: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            config_home: base.join("config/focus-music"),
            local: base.join("local.json"),
            preferences: base.join("preferences.json"),
            socket: runtime.join("mpv.sock"),
            runtime,
            state: base.join("state.json"),
            shell: base.join("config/omarchy/shell.json"),
            cache: base.join("cache"),
        };
        let sources=json!(["lofi-hiphop","piano"].into_iter().map(|genre|(genre.to_string(),json!({"label":genre,"catalog_version":format!("{genre}-v1"),"duration":160,"tracks":(0..4).map(|i|json!({"id":format!("{genre}{i}"),"url":format!("/library/{genre}{i}.wav"),"artist":"Artist","title":format!("Track {i}"),"start":i*10,"end":i*10+40,"duration":40})).collect::<Vec<_>>()}))).collect::<serde_json::Map<_,_>>());
        util::atomic_json(&paths.config_home.join("sources.json"), &sources).unwrap();
        let mock = Mock::new(&paths.socket);
        let controller = Controller::new(paths).unwrap();
        Self {
            _root: root,
            controller,
            mock,
        }
    }
    pub fn property(&self, key: &str, value: Value) {
        self.mock.props.lock().unwrap()[key] = value;
    }
}
