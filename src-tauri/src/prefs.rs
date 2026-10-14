use rusqlite::{params, Connection};
use serde_json::Value;
use std::path::Path;

pub const PREF_KEYS: &[&str] = &[
    "dragontail-app-preferences-v1",
    "dragontail-panes",
    "dragontail-vnc-connections-v1",
    "dragontail-pinned-sessions-v1",
    "dragontail-search-recent-v1",
    "dragontail-onboarding-v1",
    "dragontail-ssh-peers-v1",
];

const APP_PREFERENCES_KEY: &str = "dragontail-app-preferences-v1";

#[derive(Debug)]
pub enum PrefsError {
    UnknownKey(String),
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for PrefsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrefsError::UnknownKey(key) => write!(f, "unknown preference key: {key}"),
            PrefsError::Sqlite(err) => write!(f, "{err}"),
            PrefsError::Json(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for PrefsError {}

impl From<rusqlite::Error> for PrefsError {
    fn from(value: rusqlite::Error) -> Self {
        PrefsError::Sqlite(value)
    }
}

impl From<serde_json::Error> for PrefsError {
    fn from(value: serde_json::Error) -> Self {
        PrefsError::Json(value)
    }
}

pub struct Prefs {
    conn: Connection,
}

pub fn open(path: &Path) -> rusqlite::Result<Prefs> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS pref (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;
    Ok(Prefs { conn })
}

impl Prefs {
    pub fn get(&self, key: &str) -> Result<Option<Value>, PrefsError> {
        if !is_allowed_key(key) {
            return Err(PrefsError::UnknownKey(key.to_string()));
        }

        let mut stmt = self
            .conn
            .prepare("SELECT value FROM pref WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            let text: String = row.get(0)?;
            let value: Value = serde_json::from_str(&text)?;
            return Ok(Some(value));
        }

        if key == APP_PREFERENCES_KEY {
            return Ok(Some(default_app_preferences()));
        }

        Ok(None)
    }

    pub fn set(&self, key: &str, value: Value) -> Result<(), PrefsError> {
        if !is_allowed_key(key) {
            return Err(PrefsError::UnknownKey(key.to_string()));
        }

        let text = serde_json::to_string(&value)?;
        self.conn.execute(
            "INSERT INTO pref (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, text],
        )?;
        Ok(())
    }
}

fn is_allowed_key(key: &str) -> bool {
    PREF_KEYS.contains(&key)
}

fn default_app_preferences() -> Value {
    serde_json::json!({
        "theme": "dark",
        "uiSize": "standard",
        "density": "standard",
        "reduceMotion": false,
        "defaultGeometry": "1600x900",
        "scaleToFit": false,
        "askPass": true,
        "viewOnly": false,
        "autoSelect": true,
        "remoteResize": false,
        "preferredEncoding": "Tight",
        "qualityLevel": 8,
        "customCompressLevel": false,
        "compressLevel": 2,
        "shared": false,
        "acceptClipboard": true,
        "sendClipboard": true,
        "terminalFontSize": 16,
        "hideOffline": false,
        "listAddress": "ipv4",
        "reopenLast": false,
        "notifySession": true,
        "notifyTaildrop": true
    })
}
