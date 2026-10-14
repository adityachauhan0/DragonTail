extern crate dragontail_lib as dragontail;

#[allow(clippy::unwrap_used)]
#[allow(clippy::field_reassign_with_default)]
#[test]
fn strips_magicdns_dot_and_hides_sharee_nodes() {
    let mut status = tslocal::Status::default();
    status.backend_state = "Running".into();
    status.version = "1.94.1".into();
    status.self_.host_name = "this-mac".into();
    status.self_.dns_name = "this-mac.tailnet.ts.net.".into();
    status.self_.os = "macOS".into();
    status.self_.tailscale_ips = vec!["100.64.0.1".into(), "fd7a:115c::1".into()];
    status.peer.insert(
        "peer-key".into(),
        tslocal::PeerStatus {
            host_name: "vps".into(),
            dns_name: "vps.tailnet.ts.net.".into(),
            os: "linux".into(),
            online: true,
            tailscale_ips: vec!["100.64.0.2".into()],
            no_file_sharing_reason: String::new(),
            ..Default::default()
        },
    );
    status.peer.insert(
        "sharee".into(),
        tslocal::PeerStatus {
            sharee_node: true,
            host_name: "hidden".into(),
            ..Default::default()
        },
    );
    status.peer.insert(
        "no-files".into(),
        tslocal::PeerStatus {
            host_name: "phone".into(),
            os: "iOS".into(),
            online: true,
            no_file_sharing_reason: "file sharing disabled".into(),
            ..Default::default()
        },
    );
    let devices = dragontail::device::devices_from_status(&status);
    assert_eq!(devices.len(), 3);
    let host = devices.iter().find(|d| d.role == "host").unwrap();
    assert_eq!(host.magic_dns, "this-mac.tailnet.ts.net");
    assert_eq!(host.tailscale_version.as_deref(), Some("1.94.1"));
    assert!(!host.can_receive_files);
    assert!(host.file_sharing_reason.is_empty());
    let peer = devices.iter().find(|d| d.name == "vps").unwrap();
    assert!(peer.can_receive_files);
    assert!(peer.file_sharing_reason.is_empty());
    let blocked = devices.iter().find(|d| d.name == "phone").unwrap();
    assert!(!blocked.can_receive_files);
    assert_eq!(blocked.file_sharing_reason, "file sharing disabled");
    assert!(!devices.iter().any(|d| d.name == "hidden"));
}
