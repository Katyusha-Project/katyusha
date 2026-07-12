//! Local registry of packages installed by Katyusha.
//!
//! Stored at /var/lib/katyusha/installed.tsv as tab-separated lines:
//! name\tversion\tsha256\tinstall_prefix
//! The format is deliberately simple and human-readable (avoids
//! pulling in a serialization crate like serde just for this).

use std::fs;
use std::path::Path;

pub const MANIFEST_PATH: &str = "/var/lib/katyusha/installed.tsv";

#[derive(Debug, Clone)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub prefix: String,
}

pub fn load() -> Vec<InstalledPackage> {
    let Ok(content) = fs::read_to_string(MANIFEST_PATH) else {
        return Vec::new();
    };

    content
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() != 4 {
                return None;
            }
            Some(InstalledPackage {
                name: parts[0].to_string(),
                version: parts[1].to_string(),
                sha256: parts[2].to_string(),
                prefix: parts[3].to_string(),
            })
        })
        .collect()
}

pub fn save(packages: &[InstalledPackage]) -> Result<(), String> {
    if let Some(parent) = Path::new(MANIFEST_PATH).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content: String = packages
        .iter()
        .map(|p| format!("{}\t{}\t{}\t{}\n", p.name, p.version, p.sha256, p.prefix))
        .collect();
    fs::write(MANIFEST_PATH, content).map_err(|e| e.to_string())
}

pub fn add(pkg: InstalledPackage) -> Result<(), String> {
    let mut all = load();
    all.retain(|p| p.name != pkg.name);
    all.push(pkg);
    save(&all)
}

pub fn remove(name: &str) -> Result<Option<InstalledPackage>, String> {
    let mut all = load();
    let idx = all.iter().position(|p| p.name == name);
    let removed = idx.map(|i| all.remove(i));
    save(&all)?;
    Ok(removed)
}

pub fn find<'a>(packages: &'a [InstalledPackage], name: &str) -> Option<&'a InstalledPackage> {
    packages.iter().find(|p| p.name == name)
}
