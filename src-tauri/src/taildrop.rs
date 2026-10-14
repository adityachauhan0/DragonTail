use std::io::Read;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaildropEvent {
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes: Option<u64>,
}

struct Transfer {
    child: Child,
    id: u64,
    cancelled: bool,
}

pub struct Taildrop {
    current: Mutex<Option<Transfer>>,
    next_id: AtomicU64,
}

impl Taildrop {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn cancel(&self) -> Result<(), String> {
        let mut guard = self.current.lock().map_err(|err| err.to_string())?;
        if let Some(transfer) = guard.as_mut() {
            transfer.cancelled = true;
            let _ = transfer.child.kill();
            let _ = transfer.child.wait();
        }
        *guard = None;
        Ok(())
    }
}

impl Default for Taildrop {
    fn default() -> Self {
        Self::new()
    }
}

pub fn require_file(path: &str) -> Result<u64, String> {
    let meta = std::fs::metadata(path).map_err(|err| err.to_string())?;
    if !meta.is_file() {
        return Err("path is not a file".into());
    }
    Ok(meta.len())
}

pub async fn send(app: AppHandle, device_id: String, path: String) -> Result<(), String> {
    let bytes = require_file(&path)?;
    let snapshot = crate::tailnet::snapshot().await?;
    let device = snapshot
        .devices
        .iter()
        .find(|device| device.id == device_id)
        .ok_or_else(|| "device not found".to_string())?;
    if device.role == "host" {
        return Err("this computer cannot receive a Taildrop from itself".into());
    }
    if !device.can_receive_files {
        return Err(if device.file_sharing_reason.is_empty() {
            "this computer cannot receive files".to_string()
        } else {
            device.file_sharing_reason.clone()
        });
    }
    let target = device.name.clone();
    let file_name = std::path::Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or(path.clone());

    {
        let state = app.state::<Taildrop>();
        let guard = state.current.lock().map_err(|err| err.to_string())?;
        if guard.is_some() {
            return Err("a transfer is already running".into());
        }
    }
    let mut child = spawn_copy(&path, &target)?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_handle = std::thread::spawn(move || read_to_string(stdout));
    let stderr_handle = std::thread::spawn(move || read_to_string(stderr));
    let id = {
        let state = app.state::<Taildrop>();
        let mut guard = state.current.lock().map_err(|err| err.to_string())?;
        if guard.is_some() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("a transfer is already running".into());
        }
        let id = state.next_id.fetch_add(1, Ordering::Relaxed);
        *guard = Some(Transfer {
            child,
            id,
            cancelled: false,
        });
        id
    };

    if let Err(err) = app.emit(
        "taildrop:progress",
        TaildropEvent {
            state: "transferring".into(),
            name: Some(file_name),
            message: None,
            bytes: Some(bytes),
        },
    ) {
        let _ = app.state::<Taildrop>().cancel();
        return Err(err.to_string());
    }

    let app_watch = app.clone();
    std::thread::spawn(move || watch_transfer(app_watch, id, stdout_handle, stderr_handle));
    Ok(())
}

fn file_cp_target(name: &str) -> String {
    if name.ends_with(':') {
        name.to_string()
    } else {
        format!("{name}:")
    }
}

fn spawn_copy(path: &str, target: &str) -> Result<Child, String> {
    let destination = file_cp_target(target);
    match crate::tailscale_cli::command()
        .args(["file", "cp", path, &destination])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => Ok(child),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err("missing-binary".into()),
        Err(err) => Err(err.to_string()),
    }
}

fn read_to_string(pipe: Option<impl Read>) -> String {
    let mut text = String::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_string(&mut text);
    }
    text
}

fn watch_transfer(
    app: AppHandle,
    id: u64,
    stdout: std::thread::JoinHandle<String>,
    stderr: std::thread::JoinHandle<String>,
) {
    let outcome = loop {
        let step = {
            let state = app.state::<Taildrop>();
            let mut guard = match state.current.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let Some(transfer) = guard.as_mut() else {
                return;
            };
            if transfer.id != id {
                return;
            }
            if transfer.cancelled {
                return;
            }
            match transfer.child.try_wait() {
                Ok(Some(status)) => {
                    let cancelled = transfer.cancelled;
                    let success = status.success();
                    let code = status.code().unwrap_or(1);
                    *guard = None;
                    Some((cancelled, success, code))
                }
                Ok(None) => None,
                Err(err) => {
                    *guard = None;
                    let _ = app.emit(
                        "taildrop:progress",
                        TaildropEvent {
                            state: "error".into(),
                            name: None,
                            message: Some(err.to_string()),
                            bytes: None,
                        },
                    );
                    return;
                }
            }
        };
        if let Some(done) = step {
            break done;
        }
        std::thread::sleep(Duration::from_millis(80));
    };
    let (cancelled, success, code) = outcome;
    if cancelled {
        return;
    }
    let stdout_text = stdout.join().unwrap_or_default();
    let stderr_text = stderr.join().unwrap_or_default();
    let message = format!("{stdout_text}{stderr_text}");
    let event = if success {
        TaildropEvent {
            state: "done".into(),
            name: None,
            message: None,
            bytes: None,
        }
    } else {
        TaildropEvent {
            state: "error".into(),
            name: None,
            message: Some(if message.trim().is_empty() {
                format!("tailscale file cp exited {code}")
            } else {
                message.trim().to_string()
            }),
            bytes: None,
        }
    };
    let _ = app.emit("taildrop:progress", event);
}

pub fn cancel(app: &AppHandle) -> Result<(), String> {
    app.state::<Taildrop>().cancel()?;
    app.emit(
        "taildrop:progress",
        TaildropEvent {
            state: "idle".into(),
            name: None,
            message: None,
            bytes: None,
        },
    )
    .map_err(|err| err.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{file_cp_target, require_file};

    #[test]
    fn target_ends_with_colon() {
        assert_eq!(file_cp_target("vps"), "vps:");
        assert_eq!(file_cp_target("vps:"), "vps:");
    }

    #[test]
    fn rejects_a_directory() {
        let err = require_file("/tmp").expect_err("directory");
        assert_eq!(err, "path is not a file");
    }
}
