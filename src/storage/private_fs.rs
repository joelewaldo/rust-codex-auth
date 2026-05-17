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
    fs::write(path, bytes)?;
    set_file_permissions(path)
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

    fs::write(path, bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(existing_mode.unwrap_or(0o600)),
        )?;
    }

    Ok(())
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
fn set_file_permissions(path: &Path) -> Result<(), io::Error> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_file_permissions(_path: &Path) -> Result<(), io::Error> {
    Ok(())
}
