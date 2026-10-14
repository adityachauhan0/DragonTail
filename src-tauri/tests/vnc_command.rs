#![allow(clippy::unwrap_used)]

extern crate dragontail_lib as dragontail;

use dragontail::vnc_command::ViewerOptions;

fn sample(auto_select: bool) -> ViewerOptions {
    ViewerOptions {
        auto_select,
        view_only: false,
        shared: false,
        remote_resize: false,
        accept_clipboard: true,
        send_clipboard: true,
        preferred_encoding: "Tight".into(),
        quality_level: 8,
        custom_compress_level: false,
        compress_level: 2,
        desktop_size: "1600x900".into(),
        host: "100.64.0.2".into(),
        port: 5901,
    }
}

#[test]
fn includes_desktop_size_and_skips_encoding_when_autoselect_is_on() {
    let argv = dragontail::vnc_command::viewer_argv(&sample(true));
    assert!(argv.contains(&"-DesktopSize=1600x900".into()));
    assert!(!argv.iter().any(|arg| arg.starts_with("-PreferredEncoding")));
    assert_eq!(argv.last().unwrap(), "100.64.0.2::5901");
}

#[test]
fn brackets_ipv6_and_adds_encoding_when_autoselect_is_off() {
    let mut options = sample(false);
    options.host = "fd7a:115c::2".into();
    let argv = dragontail::vnc_command::viewer_argv(&options);
    assert!(argv.iter().any(|arg| arg == "-PreferredEncoding=Tight"));
    assert_eq!(argv.last().unwrap(), "[fd7a:115c::2]::5901");
}

#[test]
fn rejects_a_bad_desktop_size() {
    let mut options = sample(true);
    options.desktop_size = "800x600".into();
    let err = dragontail::vnc_command::vnc_argv(&options).unwrap_err();
    assert!(err.contains("desktop size"));
}
