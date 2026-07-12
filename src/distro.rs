//! Linux distribution detection by reading /etc/os-release.
//!
//! Katyusha uses this information to: (a) display it in diagnostics,
//! and (b) decide, if the index requires it in the future, which
//! package variant to install (e.g. different binaries per distro
//! family).

use std::fs;

#[derive(Debug, Clone)]
pub struct DistroInfo {
    pub id: String,           // e.g. "arch", "ubuntu", "fedora"
    pub id_like: Vec<String>, // e.g. ["debian"] in the case of ubuntu
    pub name: String,         // pretty name, e.g. "Ubuntu"
    pub version: String,      // e.g. "24.04"
    pub family: DistroFamily,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistroFamily {
    Debian,
    RedHat,
    Arch,
    Suse,
    Alpine,
    Gentoo,
    Unknown,
}

impl DistroFamily {
    fn classify(id: &str, id_like: &[String]) -> DistroFamily {
        let all: Vec<&str> = std::iter::once(id)
            .chain(id_like.iter().map(|s| s.as_str()))
            .collect();

        for tag in &all {
            match *tag {
                "debian" | "ubuntu" => return DistroFamily::Debian,
                "rhel" | "fedora" | "centos" => return DistroFamily::RedHat,
                "arch" | "archlinux" | "manjaro" => return DistroFamily::Arch,
                "suse" | "opensuse" => return DistroFamily::Suse,
                "alpine" => return DistroFamily::Alpine,
                "gentoo" => return DistroFamily::Gentoo,
                _ => {}
            }
        }
        DistroFamily::Unknown
    }
}

/// Reads and parses /etc/os-release. If anything fails, returns an
/// "unknown" distro instead of aborting: Katyusha should degrade
/// gracefully on non-standard systems (minimal containers, etc.).
pub fn detect() -> DistroInfo {
    let content = fs::read_to_string("/etc/os-release").unwrap_or_default();

    let mut id = String::from("unknown");
    let mut id_like = Vec::new();
    let mut name = String::from("Unknown");
    let mut version = String::from("?");

    for line in content.lines() {
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let value = raw_value.trim().trim_matches('"').to_string();

        match key {
            "ID" => id = value,
            "ID_LIKE" => id_like = value.split_whitespace().map(String::from).collect(),
            "NAME" => name = value,
            "VERSION_ID" => version = value,
            _ => {}
        }
    }

    let family = DistroFamily::classify(&id, &id_like);

    DistroInfo {
        id,
        id_like,
        name,
        version,
        family,
    }
}

impl std::fmt::Display for DistroInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} ({:?})", self.name, self.version, self.family)
    }
}
