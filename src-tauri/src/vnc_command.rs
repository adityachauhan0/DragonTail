use serde::Deserialize;

const DESKTOP_SIZES: &[&str] = &["1280x720", "1600x900", "1920x1080"];
const ENCODINGS: &[&str] = &["Tight", "JPEG", "ZRLE", "Hextile", "H.264", "Raw"];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerOptions {
    pub auto_select: bool,
    pub view_only: bool,
    pub shared: bool,
    pub remote_resize: bool,
    pub accept_clipboard: bool,
    pub send_clipboard: bool,
    pub preferred_encoding: String,
    pub quality_level: u8,
    pub custom_compress_level: bool,
    pub compress_level: u8,
    pub desktop_size: String,
    pub host: String,
    pub port: u16,
}

pub fn viewer_argv(options: &ViewerOptions) -> Vec<String> {
    let mut argv = vec![
        "vncviewer".to_string(),
        flag("AutoSelect", options.auto_select),
        flag("ViewOnly", options.view_only),
        flag("Shared", options.shared),
        flag("RemoteResize", options.remote_resize),
        flag("AcceptClipboard", options.accept_clipboard),
        flag("SendClipboard", options.send_clipboard),
        format!("-DesktopSize={}", options.desktop_size),
    ];
    if !options.auto_select {
        argv.push(format!(
            "-PreferredEncoding={}",
            options.preferred_encoding
        ));
        argv.push(format!("-QualityLevel={}", options.quality_level));
    }
    if options.custom_compress_level {
        argv.push("-CustomCompressLevel=1".to_string());
        argv.push(format!("-CompressLevel={}", options.compress_level));
    }
    argv.push(format!("{}::{}", host_token(&options.host), options.port));
    argv
}

pub fn vnc_argv(options: &ViewerOptions) -> Result<Vec<String>, String> {
    if !DESKTOP_SIZES.contains(&options.desktop_size.as_str()) {
        return Err("desktop size must be 1280x720, 1600x900, or 1920x1080".into());
    }
    if !ENCODINGS.contains(&options.preferred_encoding.as_str()) {
        return Err("encoding must be Tight, JPEG, ZRLE, Hextile, H.264, or Raw".into());
    }
    if options.quality_level > 9 || options.compress_level > 9 {
        return Err("quality and compress levels must be 0 through 9".into());
    }
    if options.port == 0 {
        return Err("port is required".into());
    }
    if options.host.is_empty()
        || options.host.starts_with('-')
        || options.host.chars().any(char::is_whitespace)
    {
        return Err("host is required".into());
    }
    Ok(viewer_argv(options))
}

fn flag(name: &str, on: bool) -> String {
    format!("-{name}={}", u8::from(on))
}

fn host_token(host: &str) -> String {
    if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}
