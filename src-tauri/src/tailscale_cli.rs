use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Locations a Finder-launched app can see. A `.app` does not inherit a shell PATH,
/// so Homebrew and the Tailscale app are invisible to `Command::new("tailscale")`.
const FIXED_CANDIDATES: &[&str] = &[
    "/opt/homebrew/bin/tailscale",
    "/usr/local/bin/tailscale",
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
];

pub fn command() -> Command {
    match resolve(std::env::var_os("PATH").as_deref(), fixed_candidates()) {
        Some(path) => Command::new(path),
        None => Command::new("tailscale"),
    }
}

fn fixed_candidates() -> Vec<PathBuf> {
    FIXED_CANDIDATES.iter().map(PathBuf::from).collect()
}

pub fn resolve(path_var: Option<&OsStr>, fixed: Vec<PathBuf>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path_var) = path_var {
        for dir in std::env::split_paths(path_var) {
            candidates.push(dir.join("tailscale"));
        }
    }
    candidates.extend(fixed);
    candidates.into_iter().find(|path| is_executable(path))
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn prefers_path_then_known_installs() {
        let dir = tempfile::tempdir().unwrap();
        let on_path = dir.path().join("bin");
        std::fs::create_dir(&on_path).unwrap();
        let path_bin = on_path.join("tailscale");
        std::fs::write(&path_bin, b"path").unwrap();
        make_executable(&path_bin);

        let app = dir.path().join("Tailscale");
        std::fs::write(&app, b"app").unwrap();
        make_executable(&app);

        let found = resolve(Some(on_path.as_os_str()), vec![app.clone()]).unwrap();
        assert_eq!(found, path_bin);

        let fallback = resolve(Some(dir.path().join("empty").as_os_str()), vec![app.clone()]).unwrap();
        assert_eq!(fallback, app);
    }

    #[test]
    fn missing_binary_when_nothing_is_executable() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("tailscale");
        std::fs::write(&plain, b"nope").unwrap();
        assert!(resolve(None, vec![plain]).is_none());
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }
}
