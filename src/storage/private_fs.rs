use std::{fs, io, path::Path};

pub(super) fn ensure_private_dir(path: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(path)?;
    set_dir_permissions(path)?;
    if let Some(parent) = path.parent() {
        set_dir_permissions(parent)?;
    }
    Ok(())
}

pub(super) fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), io::Error> {
    write_file_with_mode(path, bytes, Some(0o600))
}

pub(super) fn write_preserving_existing_permissions(
    path: &Path,
    bytes: &[u8],
) -> Result<(), io::Error> {
    #[cfg(unix)]
    let existing_mode = fs::metadata(path).ok().map(|metadata| {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o777
    });

    #[cfg(unix)]
    return write_file_with_mode(path, bytes, Some(existing_mode.unwrap_or(0o600)));

    #[cfg(not(unix))]
    {
        fs::write(path, bytes)
    }
}

#[cfg(unix)]
fn set_dir_permissions(path: &Path) -> Result<(), io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_dir_permissions(_path: &Path) -> Result<(), io::Error> {
    Ok(())
}

#[cfg(unix)]
fn write_file_with_mode(path: &Path, bytes: &[u8], mode: Option<u32>) -> Result<(), io::Error> {
    use std::{
        fs::OpenOptions,
        io::Write,
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
    };

    let mode = mode.unwrap_or(0o600);
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_file_with_mode(path: &Path, bytes: &[u8], _mode: Option<u32>) -> Result<(), io::Error> {
    fs::write(path, bytes)
}
