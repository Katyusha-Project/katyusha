//! Removal of packages installed by Katyusha.

use crate::{ffi, manifest};

const BIN_LINK_DIR: &str = "/usr/local/bin";

pub fn remove(name: &str) -> Result<(), String> {
    if !ffi::is_root() {
        return Err("root privileges are required. Use: sudo katyusha -r <package>".into());
    }

    let removed = manifest::remove(name)?
        .ok_or_else(|| format!("'{name}' is not installed (or wasn't installed by Katyusha)"))?;

    println!("[*] Removing {} ({})...", removed.name, removed.version);

    ffi::rm_rf(&removed.prefix)?;

    let link_path = format!("{BIN_LINK_DIR}/{name}");
    if std::path::Path::new(&link_path).exists() || std::fs::symlink_metadata(&link_path).is_ok() {
        let _ = std::fs::remove_file(&link_path);
    }

    println!("[✓] '{name}' removed successfully.");
    Ok(())
}
