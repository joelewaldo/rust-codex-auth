use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::auth::{self, AuthError, AuthInfo};

use super::{
    StorageError, accounts_dir, active_auth_path,
    model::AccountSnapshot,
    private_fs::{ensure_private_dir, write_preserving_existing_permissions, write_private_file},
    snapshot_file_name, snapshot_file_name_for_key,
};

pub fn save_active_snapshot(codex_home: &Path, info: &AuthInfo) -> Result<PathBuf, StorageError> {
    let source = active_auth_path(codex_home);
    let bytes = fs::read(&source)?;
    let dir = accounts_dir(codex_home);
    ensure_private_dir(&dir)?;
    let destination = dir.join(snapshot_file_name(info));
    write_private_file(&destination, &bytes)?;
    Ok(destination)
}

pub fn sync_active_snapshot(codex_home: &Path, info: &AuthInfo) -> Result<bool, StorageError> {
    let source = active_auth_path(codex_home);
    let source_bytes = fs::read(&source)?;
    let dir = accounts_dir(codex_home);
    ensure_private_dir(&dir)?;
    let destination = dir.join(snapshot_file_name(info));
    match fs::read(&destination) {
        Ok(existing) if existing == source_bytes => Ok(false),
        Ok(_) => {
            write_private_file(&destination, &source_bytes)?;
            Ok(true)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            write_private_file(&destination, &source_bytes)?;
            Ok(true)
        }
        Err(err) => Err(StorageError::Io(err)),
    }
}

pub fn load_accounts(codex_home: &Path) -> Result<Vec<AccountSnapshot>, StorageError> {
    let dir = accounts_dir(codex_home);
    let mut accounts = Vec::new();
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(accounts),
        Err(err) => return Err(StorageError::Io(err)),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file()
            || path.extension().and_then(|ext| ext.to_str()) != Some("json")
        {
            continue;
        }
        match auth::parse_file(&path) {
            Ok(info) => accounts.push(AccountSnapshot { info, path }),
            Err(err) => eprintln!("warning: skipping {}: {err}", path.display()),
        }
    }

    accounts.sort_by(|left, right| {
        left.info
            .email
            .cmp(&right.info.email)
            .then_with(|| left.info.account_id.cmp(&right.info.account_id))
    });
    Ok(accounts)
}

pub fn load_snapshot_by_key(codex_home: &Path, key: &str) -> Result<AccountSnapshot, StorageError> {
    let path = accounts_dir(codex_home).join(snapshot_file_name_for_key(key));
    let info = auth::parse_file(&path)?;
    Ok(AccountSnapshot { info, path })
}

pub fn active_key(codex_home: &Path) -> Result<Option<String>, StorageError> {
    let path = active_auth_path(codex_home);
    match auth::parse_file(&path) {
        Ok(info) => Ok(Some(info.record_key())),
        Err(AuthError::Io(err)) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(StorageError::Auth(err)),
    }
}

pub fn activate_account(codex_home: &Path, snapshot: &AccountSnapshot) -> Result<(), StorageError> {
    let bytes = fs::read(&snapshot.path)?;
    let destination = active_auth_path(codex_home);
    fs::create_dir_all(codex_home)?;
    write_preserving_existing_permissions(&destination, &bytes)?;
    Ok(())
}

pub fn remove_snapshot(snapshot: &AccountSnapshot) -> Result<(), StorageError> {
    fs::remove_file(&snapshot.path)?;
    Ok(())
}

pub fn remove_active_auth(codex_home: &Path) -> Result<(), StorageError> {
    match fs::remove_file(active_auth_path(codex_home)) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(StorageError::Io(err)),
    }
}

pub(super) fn active_info(codex_home: &Path) -> Result<Option<AuthInfo>, StorageError> {
    match auth::parse_file(&active_auth_path(codex_home)) {
        Ok(info) => Ok(Some(info)),
        Err(AuthError::Io(err)) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(StorageError::Auth(err)),
    }
}
