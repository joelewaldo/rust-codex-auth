mod accounts;
mod model;
mod private_fs;
mod registry;
#[cfg(test)]
mod tests;

use std::{
    env,
    ffi::OsString,
    fmt, io,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::{
    auth::{AuthError, AuthInfo},
    constants::{ACCOUNTS_DIR, APP_DIR, REGISTRY_FILE},
};

pub use accounts::{
    activate_account, active_key, load_accounts, load_snapshot_by_key, remove_active_auth,
    remove_snapshot, save_active_snapshot, sync_active_snapshot,
};
pub use model::{AccountRecord, AccountSnapshot, Registry, RolloutSignature};
pub use registry::{
    load_registry, remove_record, save_registry, set_active, sync_active_account, upsert_account,
};

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    Auth(AuthError),
    Json(serde_json::Error),
    MissingHome,
    InvalidCodexHome(PathBuf),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Auth(err) => write!(f, "{err}"),
            Self::Json(err) => write!(f, "{err}"),
            Self::MissingHome => write!(f, "could not resolve home directory"),
            Self::InvalidCodexHome(path) => {
                write!(
                    f,
                    "CODEX_HOME is not an existing directory: {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for StorageError {}

impl From<io::Error> for StorageError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<AuthError> for StorageError {
    fn from(value: AuthError) -> Self {
        Self::Auth(value)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn resolve_codex_home() -> Result<PathBuf, StorageError> {
    resolve_codex_home_from(|name| env::var_os(name))
}

pub fn resolve_codex_home_from<F>(get_var: F) -> Result<PathBuf, StorageError>
where
    F: Fn(&str) -> Option<OsString>,
{
    if let Some(value) = get_var("CODEX_HOME").filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        if path.is_dir() {
            return Ok(path);
        }
        return Err(StorageError::InvalidCodexHome(path));
    }

    if let Some(home) = get_var("HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home).join(".codex"));
    }
    if let Some(home) = get_var("USERPROFILE").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home).join(".codex"));
    }

    Err(StorageError::MissingHome)
}

pub fn active_auth_path(codex_home: &Path) -> PathBuf {
    codex_home.join("auth.json")
}

pub fn accounts_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(APP_DIR).join(ACCOUNTS_DIR)
}

pub fn registry_path(codex_home: &Path) -> PathBuf {
    codex_home.join(APP_DIR).join(REGISTRY_FILE)
}

pub fn snapshot_file_name(info: &AuthInfo) -> String {
    snapshot_file_name_for_key(&info.record_key())
}

pub fn snapshot_file_name_for_key(key: &str) -> String {
    let encoded = URL_SAFE_NO_PAD.encode(key);
    format!("{encoded}.auth.json")
}
