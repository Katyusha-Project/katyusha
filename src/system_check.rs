//! Detection of software already installed on the system through
//! means other than Katyusha (APT/dpkg, RPM, pacman, apk, Flatpak,
//! Snap, or simply already present in PATH).
//!
//! This runs before every install so Katyusha never clobbers or
//! duplicates something the user already has — e.g. if GNU nano was
//! installed with `apt install nano`, `katyusha -i nano` recognizes
//! that and skips the install instead of fighting the system package
//! manager for ownership of the binary.

use std::process::Command;

#[derive(Debug, Clone)]
pub struct ExistingInstall {
    pub source: String,
    pub detail: String,
}

impl std::fmt::Display for ExistingInstall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.source, self.detail)
    }
}

/// Runs `cmd` with `args` and returns true if it exits successfully.
/// If the binary itself is not installed, `Command` fails to spawn
/// and this simply returns false — no special-casing needed per
/// package manager.
fn succeeds(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Returns the resolved path of `name` if it's already reachable via
/// PATH (works regardless of which tool put it there).
fn check_path(name: &str) -> Option<String> {
    let output = Command::new("which").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

/// Checks every package manager Katyusha knows how to query. Each
/// check is a no-op if that manager isn't installed on this system,
/// so it's safe to run them all unconditionally.
pub fn already_installed(name: &str) -> Option<ExistingInstall> {
    // APT / Debian family
    if succeeds("dpkg", &["-s", name]) {
        return Some(ExistingInstall {
            source: "APT/dpkg".to_string(),
            detail: format!("package '{name}' is registered in dpkg's database"),
        });
    }

    // RPM-based (Fedora, RHEL, openSUSE, ...)
    if succeeds("rpm", &["-q", name]) {
        return Some(ExistingInstall {
            source: "RPM".to_string(),
            detail: format!("package '{name}' is registered in the RPM database"),
        });
    }

    // Arch / pacman
    if succeeds("pacman", &["-Q", name]) {
        return Some(ExistingInstall {
            source: "pacman".to_string(),
            detail: format!("package '{name}' is registered in pacman's local database"),
        });
    }

    // Alpine
    if succeeds("apk", &["info", "-e", name]) {
        return Some(ExistingInstall {
            source: "apk".to_string(),
            detail: format!("package '{name}' is registered in apk's database"),
        });
    }

    // Flatpak (id match is exact, so this only catches exact app IDs)
    if succeeds("flatpak", &["info", name]) {
        return Some(ExistingInstall {
            source: "Flatpak".to_string(),
            detail: format!("'{name}' is installed as a Flatpak app"),
        });
    }

    // Snap
    if succeeds("snap", &["list", name]) {
        return Some(ExistingInstall {
            source: "Snap".to_string(),
            detail: format!("'{name}' is installed as a snap"),
        });
    }

    // Last resort: something with this exact name is already
    // reachable on PATH, regardless of how it got there.
    if let Some(path) = check_path(name) {
        return Some(ExistingInstall {
            source: "PATH".to_string(),
            detail: format!("an executable named '{name}' already exists at {path}"),
        });
    }

    None
}
