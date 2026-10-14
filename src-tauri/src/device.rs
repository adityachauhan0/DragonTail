use serde::{Deserialize, Serialize};

use crate::tailnet::{PeerStatus, Status};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub role: String,
    pub name: String,
    pub os: String,
    pub online: bool,
    pub magic_dns: String,
    pub ipv4: String,
    pub ipv6: String,
    pub key_expiry: Option<String>,
    pub tailscale_version: Option<String>,
    pub can_receive_files: bool,
    pub file_sharing_reason: String,
}

pub fn devices_from_status(status: &Status) -> Vec<Device> {
    let mut devices = Vec::new();
    devices.push(peer_to_device(
        "host",
        "host",
        &status.self_,
        status.backend_state == "Running",
        true,
        Some(status.version.clone()),
    ));

    for (id, peer) in &status.peer {
        if peer.sharee_node {
            continue;
        }
        devices.push(peer_to_device(
            id,
            "remote",
            peer,
            peer.online,
            false,
            None,
        ));
    }

    devices
}

fn peer_to_device(
    id: &str,
    role: &str,
    peer: &PeerStatus,
    online: bool,
    is_host: bool,
    tailscale_version: Option<String>,
) -> Device {
    Device {
        id: id.to_string(),
        role: role.to_string(),
        name: device_name(peer),
        os: peer.os.clone(),
        online,
        magic_dns: strip_trailing_dot(&peer.dns_name),
        ipv4: first_ipv4(&peer.tailscale_ips),
        ipv6: first_ipv6(&peer.tailscale_ips),
        key_expiry: peer.key_expiry.clone(),
        tailscale_version,
        can_receive_files: !is_host && peer.no_file_sharing_reason.is_empty(),
        file_sharing_reason: if is_host {
            String::new()
        } else {
            peer.no_file_sharing_reason.clone()
        },
    }
}

fn device_name(peer: &PeerStatus) -> String {
    if !peer.host_name.is_empty() {
        peer.host_name.clone()
    } else {
        strip_trailing_dot(&peer.dns_name)
    }
}

fn strip_trailing_dot(value: &str) -> String {
    value.strip_suffix('.').unwrap_or(value).to_string()
}

fn first_ipv4(ips: &[String]) -> String {
    ips.iter()
        .find(|ip| ip.contains('.') && !ip.contains(':'))
        .cloned()
        .unwrap_or_default()
}

fn first_ipv6(ips: &[String]) -> String {
    ips.iter()
        .find(|ip| ip.contains(':'))
        .cloned()
        .unwrap_or_default()
}
