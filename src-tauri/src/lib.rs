use std::sync::Mutex;

use tauri::{Emitter, Manager, RunEvent, State};

pub mod device;
pub mod prefs;
pub mod secrets;
pub mod tailscale_cli;
pub mod pty;
pub mod taildrop;
pub mod tailnet;
pub mod vnc_bridge;
pub mod vnc_command;
pub mod vnc_probe;

#[cfg(target_os = "macos")]
mod titlebar_drag;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }));
    }
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            let db_path = config_dir.join("dragontail.sqlite");
            let store = prefs::open(&db_path)?;
            app.manage(Mutex::new(store));
            app.manage(pty::PtyRegistry::new());
            app.manage(taildrop::Taildrop::new());
            app.manage(vnc_bridge::VncControl::new());

            #[cfg(desktop)]
            {
                app.handle()
                    .plugin(tauri_plugin_window_state::Builder::default().build())?;

                if app.get_webview_window("main").is_none() {
                    if let Some(main_config) = app
                        .config()
                        .app
                        .windows
                        .iter()
                        .find(|w| w.label == "main")
                    {
                        tauri::WebviewWindowBuilder::from_config(app.handle(), main_config)?
                            .build()?;
                    }
                }

                #[cfg(target_os = "macos")]
                if let Some(window) = app.get_webview_window("main") {
                    if let Ok(ns_window) = window.ns_window() {
                        titlebar_drag::install(ns_window);
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pref_get,
            pref_set,
            tailnet_snapshot,
            tailnet_ping,
            tailnet_netcheck,
            tailscale_version,
            pty::pty_open,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            titlebar_set_holes,
            vnc_probe,
            vnc_argv,
            vnc_bridge_open,
            vnc_bridge_close,
            vnc_tunnel_open,
            vnc_tunnel_close,
            taildrop_send,
            taildrop_cancel,
            secret_get,
            secret_set,
            secret_delete
        ])
        .build(tauri::generate_context!())
        .expect("error while building DragonTail")
        .run(|app_handle, event| {
            if let RunEvent::Exit = event {
                if let Some(registry) = app_handle.try_state::<pty::PtyRegistry>() {
                    registry.kill_all();
                }
                if let Some(transfer) = app_handle.try_state::<taildrop::Taildrop>() {
                    let _ = transfer.cancel();
                }
                if let Some(vnc) = app_handle.try_state::<vnc_bridge::VncControl>() {
                    vnc.close_all();
                }
            }
        });
}

#[tauri::command]
async fn tailnet_snapshot(
    app: tauri::AppHandle,
) -> Result<tailnet::TailnetSnapshot, String> {
    let snapshot = tailnet::snapshot().await?;
    app.emit("tailnet:updated", &snapshot)
        .map_err(|err| err.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn tailnet_ping(ip: String) -> Result<String, String> {
    tailnet::tailnet_ping(ip)
}

#[tauri::command]
fn tailnet_netcheck() -> Result<String, String> {
    tailnet::tailnet_netcheck()
}

#[tauri::command]
fn tailscale_version() -> Result<String, String> {
    tailnet::tailscale_cli_version()
}

#[tauri::command]
fn vnc_probe(host: String, port: u16) -> Result<bool, String> {
    vnc_probe::vnc_probe(&host, port)
}

#[tauri::command]
fn vnc_argv(options: vnc_command::ViewerOptions) -> Result<Vec<String>, String> {
    vnc_command::vnc_argv(&options)
}

#[tauri::command]
fn vnc_bridge_open(
    control: State<'_, vnc_bridge::VncControl>,
    host: String,
    port: u16,
) -> Result<u16, String> {
    control.open_bridge(&host, port)
}

#[tauri::command]
fn vnc_bridge_close(control: State<'_, vnc_bridge::VncControl>) -> Result<(), String> {
    control.close_bridge()
}

#[tauri::command]
fn vnc_tunnel_open(
    control: State<'_, vnc_bridge::VncControl>,
    user: String,
    host: String,
    remote_port: u16,
    password: String,
) -> Result<u16, String> {
    control.open_tunnel(&user, &host, remote_port, password)
}

#[tauri::command]
fn vnc_tunnel_close(control: State<'_, vnc_bridge::VncControl>) -> Result<(), String> {
    control.close_tunnel()
}

#[tauri::command]
async fn taildrop_send(
    app: tauri::AppHandle,
    device_id: String,
    path: String,
) -> Result<(), String> {
    taildrop::send(app, device_id, path).await
}

#[tauri::command]
fn taildrop_cancel(app: tauri::AppHandle) -> Result<(), String> {
    taildrop::cancel(&app)
}

#[tauri::command]
fn secret_get(account: String) -> Result<Option<String>, String> {
    secrets::get(&account)
}

#[tauri::command]
fn secret_set(account: String, password: String) -> Result<(), String> {
    secrets::set(&account, &password)
}

#[tauri::command]
fn secret_delete(account: String) -> Result<(), String> {
    secrets::delete(&account)
}

#[derive(serde::Deserialize)]
struct TitlebarHole {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[tauri::command]
fn titlebar_set_holes(height: f64, holes: Vec<TitlebarHole>) {
    #[cfg(target_os = "macos")]
    titlebar_drag::set_holes(
        height,
        holes
            .into_iter()
            .map(|hole| titlebar_drag::CssRect {
                x: hole.x,
                y: hole.y,
                width: hole.width,
                height: hole.height,
            })
            .collect(),
    );
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (height, holes);
    }
}

#[tauri::command]
fn pref_get(
    store: State<'_, Mutex<prefs::Prefs>>,
    key: String,
) -> Result<Option<serde_json::Value>, String> {
    let guard = store.lock().map_err(|err| err.to_string())?;
    guard.get(&key).map_err(|err| err.to_string())
}

#[tauri::command]
fn pref_set(
    app: tauri::AppHandle,
    store: State<'_, Mutex<prefs::Prefs>>,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    {
        let guard = store.lock().map_err(|err| err.to_string())?;
        guard.set(&key, value).map_err(|err| err.to_string())?;
    }
    app.emit("prefs:changed", key)
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    fn sample_value() -> Option<i32> {
        Some(1)
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn crate_links() {
        assert_eq!(sample_value().unwrap(), 1);
    }
}
