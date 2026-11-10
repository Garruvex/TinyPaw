//! Per-user local endpoint: short-lived `tinypaw <command>` callers forward
//! one line to the resident instance and print its one-line reply.

use std::io::{self, BufRead, BufReader, Write};
use std::sync::mpsc;
use std::time::Duration;

use interprocess::local_socket::{
    GenericNamespaced, Listener, ListenerOptions, Name, Stream, prelude::*,
};

use super::External;

fn name() -> io::Result<Name<'static>> {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".into());
    let safe: String = user
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("tinypaw-{safe}.sock").to_ns_name::<GenericNamespaced>()
}

pub enum Bind {
    Listening(Listener),
    AlreadyRunning,
}

/// Claims the endpoint, which also makes this the single resident instance.
pub fn bind() -> io::Result<Bind> {
    match ListenerOptions::new().name(name()?).create_sync() {
        Ok(listener) => Ok(Bind::Listening(listener)),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => Ok(Bind::AlreadyRunning),
        Err(e) => Err(e),
    }
}

/// Accepts commands on a background thread and forwards them to the UI thread.
pub fn serve(listener: Listener) {
    std::thread::Builder::new()
        .name("tinypaw-ipc".into())
        .spawn(move || {
            for conn in listener.incoming().filter_map(Result::ok) {
                let mut conn = BufReader::new(conn);
                let mut line = String::new();
                if conn.read_line(&mut line).is_err() {
                    continue;
                }
                let (tx, rx) = mpsc::channel();
                super::send(External::Cli {
                    line: line.trim().to_owned(),
                    reply: tx,
                });
                let answer = rx
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap_or_else(|_| "error: no reply from TinyPaw".into());
                let _ = conn.get_mut().write_all(format!("{answer}\n").as_bytes());
            }
        })
        .expect("spawn IPC thread");
}

/// Sends one command line to the resident instance and returns its reply.
pub fn request(line: &str) -> io::Result<String> {
    let mut conn = BufReader::new(Stream::connect(name()?)?);
    conn.get_mut().write_all(format!("{line}\n").as_bytes())?;
    let mut reply = String::new();
    conn.read_line(&mut reply)?;
    Ok(reply.trim_end().to_owned())
}
