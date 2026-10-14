use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtySpawn {
    pub user: String,
    pub host: String,
    pub port: u16,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PtyData {
    id: u32,
    bytes: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PtyExit {
    id: u32,
    code: u32,
}

struct PtySession {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Box<dyn Child + Send>>,
}

struct PtyInner {
    next_id: AtomicU32,
    sessions: HashMap<u32, PtySession>,
}

pub struct PtyRegistry {
    inner: Arc<Mutex<PtyInner>>,
}

impl PtyRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PtyInner {
                next_id: AtomicU32::new(1),
                sessions: HashMap::new(),
            })),
        }
    }

    pub fn kill_all(&self) {
        let ids: Vec<u32> = {
            let guard = match self.inner.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.sessions.keys().copied().collect()
        };
        for id in ids {
            let _ = kill_session(self, id);
        }
    }
}

impl Default for PtyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PtyRegistry {
    fn drop(&mut self) {
        self.kill_all();
    }
}

pub(crate) fn valid_ssh_host(host: &str) -> bool {
    if host.is_empty() || host.starts_with('-') {
        return false;
    }
    !host.chars().any(char::is_whitespace)
}

pub(crate) fn valid_ssh_user(user: &str) -> bool {
    let mut chars = user.chars();
    let first = match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c == '_' => c,
        _ => return false,
    };
    let _ = first;
    for (index, c) in chars.enumerate() {
        if index > 31 {
            return false;
        }
        if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '_' && c != '-' {
            return false;
        }
    }
    true
}

fn encode_base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((u32::from(data[i])) << 16)
            | ((u32::from(data[i + 1])) << 8)
            | u32::from(data[i + 2]);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        i += 3;
    }
    let rem = data.len() - i;
    if rem == 1 {
        let n = u32::from(data[i]) << 16;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = (u32::from(data[i]) << 16) | (u32::from(data[i + 1]) << 8);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn kill_session(registry: &PtyRegistry, id: u32) -> Result<(), String> {
    let session = {
        let mut guard = registry.inner.lock().map_err(|err| err.to_string())?;
        guard.sessions.remove(&id)
    };
    let Some(session) = session else {
        return Err(format!("unknown pty session: {id}"));
    };
    if let Ok(mut child) = session.child.lock() {
        let _ = portable_pty::ChildKiller::kill(&mut **child);
        let _ = child.wait();
    }
    Ok(())
}

#[tauri::command]
pub fn pty_open(
    app: AppHandle,
    registry: State<'_, PtyRegistry>,
    spawn: PtySpawn,
) -> Result<u32, String> {
    if !valid_ssh_user(&spawn.user) {
        return Err("invalid ssh user".to_string());
    }
    if !valid_ssh_host(&spawn.host) {
        return Err("invalid ssh host".to_string());
    }
    if spawn.port == 0 {
        return Err("invalid ssh port".to_string());
    }

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: spawn.rows.max(1),
            cols: spawn.cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| err.to_string())?;

    let mut cmd = CommandBuilder::new("/usr/bin/ssh");
    cmd.arg("-t");
    cmd.arg("-p");
    cmd.arg(spawn.port.to_string());
    cmd.arg(format!("{}@{}", spawn.user, spawn.host));
    cmd.env("TERM", "xterm-256color");

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|err| err.to_string())?;
    drop(pair.slave);

    let master = pair.master;
    let reader = master
        .try_clone_reader()
        .map_err(|err| err.to_string())?;
    let writer = master
        .take_writer()
        .map_err(|err| err.to_string())?;

    let id = {
        let mut guard = registry.inner.lock().map_err(|err| err.to_string())?;
        let id = guard.next_id.fetch_add(1, Ordering::Relaxed);
        guard.sessions.insert(
            id,
            PtySession {
                master: Mutex::new(master),
                writer: Mutex::new(writer),
                child: Mutex::new(child),
            },
        );
        id
    };

    let app_reader = app.clone();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let payload = PtyData {
                        id,
                        bytes: encode_base64(&buf[..n]),
                    };
                    let _ = app_reader.emit("pty:data", payload);
                }
                Err(_) => break,
            }
        }
    });

    let app_wait = app.clone();
    let registry_wait = Arc::clone(&registry.inner);
    std::thread::spawn(move || {
        let exit_code = loop {
            let wait_result = {
                let guard = match registry_wait.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                let Some(session) = guard.sessions.get(&id) else {
                    return;
                };
                let Ok(mut child) = session.child.lock() else {
                    return;
                };
                child.try_wait()
            };
            match wait_result {
                Ok(Some(status)) => break status.exit_code(),
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break 1,
            }
        };

        let _ = app_wait.emit(
            "pty:exit",
            PtyExit {
                id,
                code: exit_code,
            },
        );

        if let Ok(mut guard) = registry_wait.lock() {
            guard.sessions.remove(&id);
        }
    });

    Ok(id)
}

#[tauri::command]
pub fn pty_write(registry: State<'_, PtyRegistry>, id: u32, data: String) -> Result<(), String> {
    let guard = registry.inner.lock().map_err(|err| err.to_string())?;
    let session = guard
        .sessions
        .get(&id)
        .ok_or_else(|| format!("unknown pty session: {id}"))?;
    let mut writer = session.writer.lock().map_err(|err| err.to_string())?;
    writer
        .write_all(data.as_bytes())
        .map_err(|err| err.to_string())?;
    writer.flush().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn pty_resize(
    registry: State<'_, PtyRegistry>,
    id: u32,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let guard = registry.inner.lock().map_err(|err| err.to_string())?;
    let session = guard
        .sessions
        .get(&id)
        .ok_or_else(|| format!("unknown pty session: {id}"))?;
    let master = session.master.lock().map_err(|err| err.to_string())?;
    master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn pty_kill(registry: State<'_, PtyRegistry>, id: u32) -> Result<(), String> {
    kill_session(&registry, id)
}

#[cfg(test)]
mod tests {
    use super::valid_ssh_host;

    #[test]
    fn accepts_tailnet_addresses() {
        assert!(valid_ssh_host("100.64.0.2"));
        assert!(valid_ssh_host("peer.tailnet.ts.net"));
    }

    #[test]
    fn rejects_unsafe_hosts() {
        assert!(!valid_ssh_host(""));
        assert!(!valid_ssh_host("-oProxyCommand=evil"));
        assert!(!valid_ssh_host("100.1.1.1 extra"));
        assert!(!valid_ssh_host("bad\nhost"));
    }
}
