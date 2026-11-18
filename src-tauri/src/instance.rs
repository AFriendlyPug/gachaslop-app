//! Only one copy runs at a time: the first listens on a local port, and a second launch asks it to show
//! its popup, then exits.
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use crate::config::INSTANCE_PORT;
use crate::logs;

pub const SHOW: &[u8] = b"gachaslop:show\n";
pub const OK: &[u8] = b"gachaslop:ok\n";

pub enum Start {
    /// This is the only copy: serve() the listener.
    First(TcpListener),
    /// Another copy is running and has shown its popup: exit.
    AlreadyRunning,
    /// The port is used by something else: run anyway, without the one-copy guard.
    Unguarded,
}

fn address() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, INSTANCE_PORT))
}

/// Read one short message (the protocol is a single line each way).
fn read_message(stream: &mut TcpStream) -> Vec<u8> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0u8; 64];
    let n = stream.read(&mut buf).unwrap_or(0);
    buf[..n].to_vec()
}

pub fn claim() -> Start {
    if let Ok(listener) = TcpListener::bind(address()) {
        return Start::First(listener);
    }
    let shown = TcpStream::connect_timeout(&address(), Duration::from_secs(1)).ok().is_some_and(|mut stream| {
        stream.write_all(SHOW).is_ok() && read_message(&mut stream) == OK
    });
    if shown { Start::AlreadyRunning } else { Start::Unguarded }
}

/// Answer second launches, calling `show` for each.
pub fn serve(listener: TcpListener, show: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            if read_message(&mut stream) == SHOW {
                let _ = stream.write_all(OK);
                logs::info("Another launch asked for the popup");
                show();
            }
        }
    });
}
