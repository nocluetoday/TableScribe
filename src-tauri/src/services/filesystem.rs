use std::{fs, path::Path};

use crate::types::TableScribeResult;

pub fn ensure_dir(path: &Path) -> TableScribeResult<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

pub fn write_if_missing(path: &Path, contents: &str) -> TableScribeResult<()> {
    if !path.exists() {
        fs::write(path, contents)?;
    }
    Ok(())
}

pub fn copy_into_dir(
    source: &Path,
    destination_dir: &Path,
    destination_name: &str,
) -> TableScribeResult<std::path::PathBuf> {
    ensure_dir(destination_dir)?;
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("audio");
    let destination = destination_dir.join(format!("{}.{}", destination_name, extension));
    fs::copy(source, &destination)?;
    Ok(destination)
}

