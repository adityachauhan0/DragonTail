use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream as StdTcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener as TokioListener, TcpStream};
use tokio::sync::{oneshot, watch};
use tokio_tungstenite::tungstenite::Message;

use crate::pty;

pub struct Bridge {
    port: u16,
    shutdown: Option<watch::Sender<bool>>,
    thread: Option<JoinHandle<()>>,
}

impl Bridge {
    pub fn open(host: &str, port: u16) -> Result<Self, String> {
        if !valid_bridge_host(host) || port == 0 {
            return Err("host and port are required".into());
        }
        let std_listener = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
        std_listener
            .set_nonblocking(true)
            .map_err(|err| err.to_string())?;
        let listen_port = std_listener
            .local_addr()
            .map_err(|err| err.to_string())?
            .port();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (ready_tx, ready_rx) = oneshot::channel();
        let target_host = host.to_string();
        let thread = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    let _ = ready_tx.send(Err(err.to_string()));
                    return;
                }
            };
            runtime.block_on(async move {
                let listener = match TokioListener::from_std(std_listener) {
                    Ok(listener) => listener,
                    Err(err) => {
                        let _ = ready_tx.send(Err(err.to_string()));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                serve(listener, target_host, port, shutdown_rx).await;
            });
        });
        match ready_rx.blocking_recv() {
            Ok(Ok(())) => Ok(Self {
                port: listen_port,
                shutdown: Some(shutdown_tx),
                thread: Some(thread),
            }),
            Ok(Err(err)) => {
                let _ = thread.join();
                Err(err)
            }
            Err(_) => {
                let _ = thread.join();
                Err("vnc bridge stopped before it listened".into())
            }
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn close(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(true);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn serve(
    listener: TokioListener,
    host: String,
    port: u16,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            break;
        }
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            accepted_stream = listener.accept() => {
                let Ok((stream, addr)) = accepted_stream else {
                    break;
                };
                if !addr.ip().is_loopback() {
                    drop(stream);
                    continue;
                }
                match tokio_tungstenite::accept_async(stream).await {
                    Ok(socket) => {
                        proxy(socket, &host, port, shutdown.clone()).await;
                    }
                    Err(_) => continue,
                }
            }
        }
    }
}

async fn proxy(
    socket: tokio_tungstenite::WebSocketStream<TcpStream>,
    host: &str,
    port: u16,
    mut shutdown: watch::Receiver<bool>,
) {
    let tcp = match TcpStream::connect((host, port)).await {
        Ok(tcp) => tcp,
        Err(_) => return,
    };
    let (mut tcp_read, mut tcp_write) = tcp.into_split();
    let (mut ws_write, mut ws_read) = socket.split();
    let mut buf = vec![0_u8; 16 * 1024];
    loop {
        if *shutdown.borrow() {
            break;
        }
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            message = ws_read.next() => {
                match message {
                    Some(Ok(Message::Binary(bytes))) => {
                        if tcp_write.write_all(&bytes).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if tcp_write.write_all(text.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if ws_write.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            read = tcp_read.read(&mut buf) => {
                match read {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let payload = buf[..n].to_vec();
                        if ws_write.send(Message::binary(payload)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }
    let _ = ws_write.close().await;
}

fn valid_bridge_host(host: &str) -> bool {
    !host.is_empty() && !host.starts_with('-') && !host.chars().any(char::is_whitespace)
}

/// A VNC display number (`:1`) is TCP port `5900 + display`. Ports of 100 and above are already TCP ports.
pub fn tcp_port(port: u16) -> u16 {
    if port < 100 { 5900 + port } else { port }
}

pub struct Tunnel {
    _master: Box<dyn MasterPty + Send>,
    _writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Box<dyn Child + Send>>,
    local_port: u16,
}

impl Tunnel {
    pub fn open(
        user: &str,
        host: &str,
        remote_port: u16,
        password: String,
    ) -> Result<Self, String> {
        if !pty::valid_ssh_user(user) {
            return Err("invalid ssh user".into());
        }
        if !pty::valid_ssh_host(host) {
            return Err("invalid ssh host".into());
        }
        if remote_port == 0 {
            return Err("invalid vnc port".into());
        }
        let remote_port = tcp_port(remote_port);
        let probe = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
        let local_port = probe.local_addr().map_err(|err| err.to_string())?.port();
        drop(probe);
        let forward = format!("127.0.0.1:{local_port}:127.0.0.1:{remote_port}");
        let target = format!("{user}@{host}");

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| err.to_string())?;
        let mut cmd = CommandBuilder::new("/usr/bin/ssh");
        cmd.arg("-N");
        cmd.arg("-o");
        cmd.arg("ExitOnForwardFailure=yes");
        cmd.arg("-o");
        cmd.arg("ConnectTimeout=10");
        cmd.arg("-o");
        cmd.arg("StrictHostKeyChecking=accept-new");
        cmd.arg("-L");
        cmd.arg(&forward);
        cmd.arg(&target);
        cmd.env("TERM", "xterm-256color");
        cmd.env("SSH_ASKPASS_REQUIRE", "never");
        if let Some(sock) = ssh_auth_sock() {
            cmd.env("SSH_AUTH_SOCK", sock);
        }
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|err| err.to_string())?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| err.to_string())?;
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|err| err.to_string())?;
        let output = Arc::new(Mutex::new(String::new()));
        let output_reader = Arc::clone(&output);
        thread::spawn(move || {
            let mut buf = [0_u8; 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]);
                        if let Ok(mut guard) = output_reader.lock() {
                            guard.push_str(&text);
                            if guard.len() > 8_000 {
                                let extra = guard.len() - 4_000;
                                guard.drain(..extra);
                            }
                        }
                    }
                }
            }
        });

        let local_addr = format!("127.0.0.1:{local_port}")
            .parse()
            .map_err(|err: std::net::AddrParseError| err.to_string())?;
        let mut deadline = Instant::now() + Duration::from_secs(20);
        let mut seen = 0_usize;
        let mut password = password;
        let mut answered = false;
        let result = loop {
            if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
                let detail = pty_output(&output);
                break Err(ssh_failure(status.exit_code(), &detail));
            }
            if StdTcpStream::connect_timeout(&local_addr, Duration::from_millis(200)).is_ok() {
                break Ok(());
            }
            let pending = pending_output(&output, seen);
            if asks_for_password(&pending) {
                if password.is_empty() {
                    break Err(if answered {
                        "SSH password was rejected.".into()
                    } else {
                        "SSH password is required.".into()
                    });
                }
                writer
                    .write_all(password.as_bytes())
                    .map_err(|err| err.to_string())?;
                writer.write_all(b"\n").map_err(|err| err.to_string())?;
                writer.flush().map_err(|err| err.to_string())?;
                password.clear();
                answered = true;
                seen = output_len(&output);
                deadline = Instant::now() + Duration::from_secs(20);
                continue;
            }
            if Instant::now() >= deadline {
                break Err("ssh tunnel did not start listening".into());
            }
            thread::sleep(Duration::from_millis(100));
        };
        match result {
            Ok(()) => Ok(Self {
                _master: pair.master,
                _writer: Mutex::new(writer),
                child: Mutex::new(child),
                local_port,
            }),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(err)
            }
        }
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    pub fn close(mut self) {
        self.kill();
    }

    fn kill(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.kill();
    }
}

fn ssh_auth_sock() -> Option<String> {
    if let Ok(sock) = std::env::var("SSH_AUTH_SOCK") {
        if !sock.is_empty() && std::path::Path::new(&sock).exists() {
            return Some(sock);
        }
    }
    let output = Command::new("/bin/launchctl")
        .args(["getenv", "SSH_AUTH_SOCK"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sock = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if sock.is_empty() || !std::path::Path::new(&sock).exists() {
        return None;
    }
    Some(sock)
}

fn asks_for_password(text: &str) -> bool {
    text.to_ascii_lowercase().contains("password:")
}

fn output_len(output: &Mutex<String>) -> usize {
    output.lock().map(|guard| guard.len()).unwrap_or(0)
}

fn pending_output(output: &Mutex<String>, seen: usize) -> String {
    let Ok(guard) = output.lock() else {
        return String::new();
    };
    if guard.len() <= seen {
        return String::new();
    }
    guard[seen..].to_string()
}

fn pty_output(output: &Mutex<String>) -> String {
    output
        .lock()
        .map(|guard| guard.trim().to_string())
        .unwrap_or_default()
}

fn ssh_failure(code: u32, detail: &str) -> String {
    let detail = detail.trim();
    if detail.is_empty() {
        format!("ssh tunnel exited ({code})")
    } else {
        let end = detail.len();
        let start = end.saturating_sub(400);
        format!("ssh tunnel exited ({code}): {}", &detail[start..])
    }
}

pub struct VncControl {
    bridge: std::sync::Mutex<Option<Bridge>>,
    tunnel: std::sync::Mutex<Option<Tunnel>>,
}

impl Default for VncControl {
    fn default() -> Self {
        Self::new()
    }
}

impl VncControl {
    pub fn new() -> Self {
        Self {
            bridge: std::sync::Mutex::new(None),
            tunnel: std::sync::Mutex::new(None),
        }
    }

    pub fn close_all(&self) {
        let _ = self.close_bridge();
        let _ = self.close_tunnel();
    }

    pub fn open_bridge(&self, host: &str, port: u16) -> Result<u16, String> {
        let bridge = Bridge::open(host, port)?;
        let listen = bridge.port();
        let previous = {
            let mut guard = self.bridge.lock().map_err(|err| err.to_string())?;
            guard.replace(bridge)
        };
        if let Some(previous) = previous {
            previous.close();
        }
        Ok(listen)
    }

    pub fn close_bridge(&self) -> Result<(), String> {
        let previous = self
            .bridge
            .lock()
            .map_err(|err| err.to_string())?
            .take();
        if let Some(bridge) = previous {
            bridge.close();
        }
        Ok(())
    }

    pub fn open_tunnel(
        &self,
        user: &str,
        host: &str,
        remote_port: u16,
        password: String,
    ) -> Result<u16, String> {
        let tunnel = Tunnel::open(user, host, remote_port, password)?;
        let local = tunnel.local_port();
        let previous = {
            let mut guard = self.tunnel.lock().map_err(|err| err.to_string())?;
            guard.replace(tunnel)
        };
        if let Some(previous) = previous {
            previous.close();
        }
        Ok(local)
    }

    pub fn close_tunnel(&self) -> Result<(), String> {
        let previous = self
            .tunnel
            .lock()
            .map_err(|err| err.to_string())?
            .take();
        if let Some(tunnel) = previous {
            tunnel.close();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Bridge, Tunnel};
    use futures_util::{SinkExt, StreamExt};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use tokio_tungstenite::tungstenite::Message;

    #[test]
    fn bridge_echoes_ping_then_stops() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("echo bind");
        let echo_port = listener.local_addr().expect("echo addr").port();
        let accepts = Arc::new(AtomicUsize::new(0));
        let accepts_thread = Arc::clone(&accepts);
        thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                accepts_thread.fetch_add(1, Ordering::SeqCst);
                thread::spawn(move || {
                    let mut buf = [0_u8; 64];
                    loop {
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                if stream.write_all(&buf[..n]).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
            }
        });

        let bridge = Bridge::open("127.0.0.1", echo_port).expect("bridge");
        let listen_port = bridge.port();
        let url = format!("ws://127.0.0.1:{listen_port}");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let (mut socket, _) = tokio_tungstenite::connect_async(url)
                .await
                .expect("websocket");
            socket
                .send(Message::binary(b"ping".as_slice()))
                .await
                .expect("send");
            let message = socket.next().await.expect("frame").expect("message");
            assert_eq!(message.into_data(), b"ping".as_slice());
        });
        assert_eq!(accepts.load(Ordering::SeqCst), 1);

        bridge.close();
        thread::sleep(Duration::from_millis(150));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let refused = runtime.block_on(async {
            tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{listen_port}"))
                .await
                .is_err()
        });
        assert!(refused);
        thread::sleep(Duration::from_millis(150));
        assert_eq!(accepts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn display_number_maps_to_the_tcp_port() {
        assert_eq!(super::tcp_port(1), 5901);
        assert_eq!(super::tcp_port(0), 5900);
        assert_eq!(super::tcp_port(5901), 5901);
    }

    #[test]
    fn password_prompt_is_detected() {
        assert!(super::asks_for_password("user@100.64.0.2's password:"));
        assert!(!super::asks_for_password("Permission denied (publickey)."));
    }

    #[test]
    fn tunnel_rejects_an_unsafe_user() {
        match Tunnel::open("-rf", "127.0.0.1", 5901, String::new()) {
            Err(err) => assert!(err.contains("user")),
            Ok(_) => panic!("expected an invalid user to be rejected"),
        }
    }
}
