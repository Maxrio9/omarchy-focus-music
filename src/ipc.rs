use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::{Duration, Instant},
};
pub struct Ipc {
    stream: BufReader<UnixStream>,
    serial: u64,
}
impl Ipc {
    pub fn connect(socket: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        Ok(Self {
            stream: BufReader::new(stream),
            serial: 0,
        })
    }
    pub fn command(&mut self, args: Value) -> Result<Value> {
        self.serial += 1;
        let serial = self.serial;
        writeln!(
            self.stream.get_mut(),
            "{}",
            json!({"command":args,"request_id":serial})
        )?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if Instant::now() > deadline {
                bail!("Player response timed out");
            }
            let mut line = String::new();
            if self.stream.read_line(&mut line)? == 0 {
                bail!("No player response");
            }
            let reply: Value = serde_json::from_str(&line)?;
            if reply["request_id"].as_u64() == Some(serial) {
                if reply["error"] != "success" {
                    bail!("Player: {}", reply["error"]);
                }
                return Ok(reply["data"].clone());
            }
        }
    }
    pub fn property(&mut self, name: &str) -> Result<Value> {
        self.command(json!(["get_property", name]))
    }
}
pub fn command(socket: &Path, args: Value) -> Result<Value> {
    Ipc::connect(socket)?.command(args)
}
pub fn property(socket: &Path, name: &str) -> Result<Value> {
    command(socket, json!(["get_property", name]))
}
