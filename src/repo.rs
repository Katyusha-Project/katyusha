//! Access to the Katyusha package repository.
//!
//! The repository is a single `index.txt` file hosted on GitHub
//! (raw.githubusercontent.com). Each line describes one package:
//!
//!   [NAME][VERSION][URL][URL_PACK][DESCRIPTION optional]
//!
//! - NAME:        package name, used in `katyusha -i <NAME>`
//! - VERSION:     package version
//! - URL:         project homepage (informational)
//! - URL_PACK:    direct download URL for the package (must be https)
//! - DESCRIPTION: optional one-line description
//!
//! `curl` is used as the HTTP client instead of a crate like `reqwest`
//! to keep Rust dependencies to a minimum and to reuse the system's
//! own TLS configuration (certificates, proxies, etc.).

use std::process::Command;

/// Default index URL. Can be overridden with the KATYUSHA_REPO_URL
/// environment variable to point at a fork or a private mirror.
pub const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/Katyusha-Project/katyusha-packages-archive/main/index.txt";

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub url: String,
    pub url_pack: String,
    pub description: Option<String>,
}

pub fn index_url() -> String {
    std::env::var("KATYUSHA_REPO_URL").unwrap_or_else(|_| DEFAULT_INDEX_URL.to_string())
}

/// Downloads index.txt using `curl` (fails explicitly if curl isn't
/// installed; it's a declared system dependency).
pub fn fetch_index() -> Result<String, String> {
    let url = index_url();

    let output = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location", &url])
        .output()
        .map_err(|e| format!("could not run curl: {e}. Is it installed?"))?;

    if !output.status.success() {
        return Err(format!(
            "curl failed to download the index ({}): {}",
            url,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout).map_err(|_| "the index is not valid UTF-8".to_string())
}

/// Parses the content of index.txt into a list of packages.
/// Empty lines and lines starting with '#' are ignored (comments).
pub fn parse_index(raw: &str) -> Vec<Package> {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(parse_line)
        .collect()
}

fn parse_line(line: &str) -> Option<Package> {
    // Extracts up to 5 fields delimited by [ ... ]
    let mut fields = Vec::with_capacity(5);
    let mut rest = line;

    while let Some(start) = rest.find('[') {
        let after_start = &rest[start + 1..];
        let end = after_start.find(']')?;
        fields.push(after_start[..end].trim().to_string());
        rest = &after_start[end + 1..];
        if fields.len() == 5 {
            break;
        }
    }

    if fields.len() < 4 {
        return None; // malformed line: missing required fields
    }

    Some(Package {
        name: fields[0].clone(),
        version: fields[1].clone(),
        url: fields[2].clone(),
        url_pack: fields[3].clone(),
        description: fields.get(4).cloned().filter(|s| !s.is_empty()),
    })
}

/// Downloads and parses the index in a single call.
pub fn load_packages() -> Result<Vec<Package>, String> {
    let raw = fetch_index()?;
    Ok(parse_index(&raw))
}

/// Finds a package by exact name (case-insensitive).
pub fn find<'a>(packages: &'a [Package], name: &str) -> Option<&'a Package> {
    packages
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_line() {
        let line = "[nano][7.2][https://nano-editor.org][https://example.com/nano.tar.gz][Simple terminal text editor]";
        let pkg = parse_line(line).unwrap();
        assert_eq!(pkg.name, "nano");
        assert_eq!(pkg.version, "7.2");
        assert_eq!(pkg.description.as_deref(), Some("Simple terminal text editor"));
    }

    #[test]
    fn parses_line_without_description() {
        let line = "[htop][3.3.0][https://htop.dev][https://example.com/htop.tar.gz]";
        let pkg = parse_line(line).unwrap();
        assert_eq!(pkg.name, "htop");
        assert!(pkg.description.is_none());
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let raw = "# comment\n\n[a][1][u][p]\n";
        let pkgs = parse_index(raw);
        assert_eq!(pkgs.len(), 1);
    }
}
