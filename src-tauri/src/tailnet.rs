use serde::{Deserialize, Serialize};

pub use tslocal::{PeerStatus, Status};

use crate::device::{self, Device};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TailnetSnapshot {
    pub backend_state: String,
    pub tailnet_name: String,
    pub account: String,
    pub version: String,
    pub devices: Vec<Device>,
}

pub async fn snapshot() -> Result<TailnetSnapshot, String> {
    let status = tslocal::Client::new()
        .status()
        .await
        .map_err(|err| err.to_string())?;
    Ok(snapshot_from_status(&status))
}

pub fn snapshot_from_status(status: &Status) -> TailnetSnapshot {
    TailnetSnapshot {
        backend_state: status.backend_state.clone(),
        tailnet_name: status
            .current_tailnet
            .as_ref()
            .map(|tailnet| tailnet.name.clone())
            .unwrap_or_default(),
        account: account_from_status(status),
        version: status.version.clone(),
        devices: device::devices_from_status(status),
    }
}

fn account_from_status(status: &Status) -> String {
    let Some(profile) = status.user.get(&status.self_.user_id) else {
        return String::new();
    };
    if !profile.display_name.is_empty() {
        profile.display_name.clone()
    } else {
        profile.login_name.clone()
    }
}

pub fn tailscale_cli_version() -> Result<String, String> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    let child = match crate::tailscale_cli::command()
        .arg("version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err("missing-binary".into());
        }
        Err(err) => return Err(err.to_string()),
    };

    let output = child.wait_with_output().map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.is_empty() {
            "tailscale version failed".into()
        } else {
            stderr.trim().to_string()
        });
    }

    let stdout = BufReader::new(output.stdout.as_slice());
    let first_line = stdout.lines().next();
    match first_line {
        Some(Ok(line)) => Ok(line.trim().to_string()),
        Some(Err(err)) => Err(err.to_string()),
        None => Ok(String::new()),
    }
}

fn run_tailscale_cli(args: &[&str]) -> Result<String, String> {
    use std::process::Stdio;

    let child = match crate::tailscale_cli::command()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err("missing-binary".into());
        }
        Err(err) => return Err(err.to_string()),
    };

    let output = child.wait_with_output().map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(format!("{stdout}{stderr}"))
}

pub fn tailnet_ping(ip: String) -> Result<String, String> {
    run_tailscale_cli(&["ping", "--c", "1", "--timeout", "3s", &ip])
}

pub fn tailnet_netcheck() -> Result<String, String> {
    run_tailscale_cli(&["netcheck"])
}
