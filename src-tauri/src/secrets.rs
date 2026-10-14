use std::process::Command;

const SERVICE: &str = "com.dragontail.app";

pub fn get(account: &str) -> Result<Option<String>, String> {
    let account = normalize_account(account)?;
    let output = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-a", &account, "-w"])
        .output()
        .map_err(|err| err.to_string())?;
    interpret_find(output.status.code(), &output.stdout, &output.stderr)
}

pub fn set(account: &str, password: &str) -> Result<(), String> {
    let account = normalize_account(account)?;
    if password.is_empty() || password.contains('\n') || password.contains('\0') {
        return Err("Password cannot be saved.".into());
    }
    let output = Command::new("/usr/bin/security")
        .args([
            "add-generic-password",
            "-U",
            "-A",
            "-s",
            SERVICE,
            "-a",
            &account,
            "-w",
            password,
        ])
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error(&output.stderr, "Could not save the password."))
    }
}

pub fn delete(account: &str) -> Result<(), String> {
    let account = normalize_account(account)?;
    let output = Command::new("/usr/bin/security")
        .args(["delete-generic-password", "-s", SERVICE, "-a", &account])
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() || output.status.code() == Some(44) {
        Ok(())
    } else {
        Err(command_error(&output.stderr, "Could not forget the password."))
    }
}

fn normalize_account(account: &str) -> Result<String, String> {
    let account = account.trim();
    if account.is_empty()
        || account.len() > 200
        || account.chars().any(|ch| ch.is_control())
    {
        return Err("Password account is invalid.".into());
    }
    Ok(account.to_string())
}

fn interpret_find(code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<Option<String>, String> {
    if code == Some(0) {
        let mut password = String::from_utf8_lossy(stdout).into_owned();
        if password.ends_with('\n') {
            password.pop();
            if password.ends_with('\r') {
                password.pop();
            }
        }
        return Ok(Some(password));
    }
    if code == Some(44) {
        return Ok(None);
    }
    Err(command_error(stderr, "Could not read the saved password."))
}

fn command_error(stderr: &[u8], fallback: &str) -> String {
    let text = String::from_utf8_lossy(stderr);
    let line = text.lines().find(|line| !line.trim().is_empty());
    match line {
        Some(line) => line.trim().to_string(),
        None => fallback.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn missing_item_is_empty() {
        let found = interpret_find(Some(44), b"", b"item not found").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn found_password_drops_only_the_trailing_newline() {
        let found = interpret_find(Some(0), b"secret\n", b"").unwrap();
        assert_eq!(found.as_deref(), Some("secret"));
    }

    #[test]
    fn account_rejects_control_characters() {
        assert!(normalize_account("ssh:user@100.64.0.1").is_ok());
        assert!(normalize_account("ssh:bad\nhost").is_err());
    }
}
