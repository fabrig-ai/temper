//! Real inherited-fd handoffs without unsafe pre-exec hooks.
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    os::fd::{AsRawFd, OwnedFd},
    process::{Command, ExitStatus, Stdio},
    time::Duration,
};

#[must_use]
pub fn command(listener: TcpListener, script: &str) -> Command {
    listener.set_nonblocking(true).expect("nonblocking listener");
    let mut command = Command::new("/bin/sh");
    // stdin carries the socket only to the shell; dup to 3 clears CLOEXEC,
    // exactly as the box's Given does, before any temper startup opens.
    command
        .args(["-c", script, "inherited-listener", env!("CARGO_BIN_EXE_temper")])
        .env_clear()
        .env("LISTEN_FDS", "1")
        .stdin(Stdio::from(OwnedFd::from(listener)))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

pub struct Probed {
    pub ready: Option<String>,
    pub reply: io::Result<Vec<u8>>,
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[must_use]
pub fn probe(mut command: Command, address: SocketAddr, marker: &str) -> Probed {
    let mut child = command.spawn().expect("listener claimant");
    let clock = skein_shell::Clock::new();
    let deadline = clock.now().now.saturating_add(skein_lib::Duration::from_secs(4));
    let mut stdout = Vec::new();
    let mut ready = None;
    loop {
        let pipe = child.stdout.as_mut().expect("stdout pipe");
        let left = deadline.saturating_since(clock.now().now);
        if !skein_shell::wait_readable(skein_io::kernel::Fd::new(pipe.as_raw_fd()), left).expect("stdout readiness") {
            break;
        }
        let mut bytes = [0; 1024];
        let count = pipe.read(&mut bytes).expect("stdout read");
        if count == 0 {
            break;
        }
        stdout.extend_from_slice(&bytes[..count]);
        let text = String::from_utf8_lossy(&stdout);
        if text.contains(marker) {
            ready = Some(text.into_owned());
            break;
        }
        if clock.now().now >= deadline || stdout.len() > 65_536 {
            break;
        }
    }
    let reply = if ready.is_some() { request(address) } else { Err(io::ErrorKind::TimedOut.into()) };
    let _signalled = Command::new("/usr/bin/kill").args(["-TERM", &child.id().to_string()]).output();
    let output = child.wait_with_output().expect("claimant reaped");
    stdout.extend_from_slice(&output.stdout);
    Probed { ready, reply, status: output.status, stdout, stderr: output.stderr }
}

fn request(address: SocketAddr) -> io::Result<Vec<u8>> {
    let mut socket = TcpStream::connect_timeout(&address, Duration::from_millis(500))?;
    socket.set_read_timeout(Some(Duration::from_millis(500)))?;
    socket.set_write_timeout(Some(Duration::from_millis(500)))?;
    socket.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut bytes = [0; 32];
    let count = socket.read(&mut bytes)?;
    Ok(bytes[..count].to_vec())
}
