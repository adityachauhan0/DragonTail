#![allow(clippy::unwrap_used)]

use std::net::TcpListener;

extern crate dragontail_lib as dragontail;

#[test]
fn open_port_is_true_and_closed_port_is_false() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    assert!(dragontail::vnc_probe::vnc_probe("127.0.0.1", port).expect("probe open"));
    drop(listener);
    assert!(!dragontail::vnc_probe::vnc_probe("127.0.0.1", port).expect("probe closed"));
}
