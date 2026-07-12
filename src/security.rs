//! Katyusha's security policies.
//!
//! This is not an antivirus and doesn't pretend to be one — it won't
//! catch a malicious binary that's otherwise served correctly. What
//! it does close off is the boring stuff that actually gets people
//! hacked: HTTP downgrade, typosquatted hosts, a file getting swapped
//! out from under a package after the fact, and tar path-traversal
//! tricks. HTTPS-only, a trusted host allowlist, TOFU checksums, and
//! an extraction audit before anything touches the real filesystem.

use std::path::Path;

/// Hosts trusted by default (github.com/githubusercontent, where the
/// index and packages both live). Extend this in
/// ~/.config/katyusha/trusted_hosts.txt
const DEFAULT_TRUSTED_HOSTS: &[&str] = &[
    "github.com",
    "raw.githubusercontent.com",
    "objects.githubusercontent.com",
    "codeload.github.com",
];

#[derive(Debug)]
pub enum SecurityError {
    InsecureScheme(String),
    UntrustedHost(String),
    ChecksumMismatch { expected: String, actual: String },
    PathTraversal(String),
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityError::InsecureScheme(url) => {
                write!(f, "insecure URL (https is required): {url}")
            }
            SecurityError::UntrustedHost(host) => write!(
                f,
                "untrusted host '{host}'. Add it to ~/.config/katyusha/trusted_hosts.txt if you trust it"
            ),
            SecurityError::ChecksumMismatch { expected, actual } => write!(
                f,
                "SHA-256 mismatch (expected {expected}, got {actual}). \
                 The file may have been tampered with."
            ),
            SecurityError::PathTraversal(p) => {
                write!(f, "unsafe path detected inside the package: {p}")
            }
        }
    }
}

pub fn require_https(url: &str) -> Result<(), SecurityError> {
    if url.starts_with("https://") {
        Ok(())
    } else {
        Err(SecurityError::InsecureScheme(url.to_string()))
    }
}

/// Pulls the host out of an https:// URL without pulling in a full
/// URL-parsing crate for it.
fn extract_host(url: &str) -> Option<&str> {
    let without_scheme = url.strip_prefix("https://")?;
    let end = without_scheme.find(['/', ':']).unwrap_or(without_scheme.len());
    Some(&without_scheme[..end])
}

pub fn check_trusted_host(url: &str, extra_trusted: &[String]) -> Result<(), SecurityError> {
    let host = extract_host(url).ok_or_else(|| SecurityError::InsecureScheme(url.to_string()))?;

    let trusted = DEFAULT_TRUSTED_HOSTS.iter().any(|h| *h == host)
        || extra_trusted.iter().any(|h| h == host);

    if trusted {
        Ok(())
    } else {
        Err(SecurityError::UntrustedHost(host.to_string()))
    }
}

pub fn load_extra_trusted_hosts() -> Vec<String> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let path = Path::new(&home).join(".config/katyusha/trusted_hosts.txt");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

pub fn verify_checksum(actual: &str, expected: &str) -> Result<(), SecurityError> {
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(SecurityError::ChecksumMismatch {
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

/// Walks an already-extracted directory looking for symlinks that
/// escape it, or ".." in an entry name that somehow survived tar's
/// own extraction guard. Modern tar already blocks this on its own;
/// this is just a second check before anything gets copied further.
pub fn audit_extracted_dir(root: &Path) -> Result<(), SecurityError> {
    let root_canon = root
        .canonicalize()
        .map_err(|_| SecurityError::PathTraversal(root.display().to_string()))?;

    for entry in walk(root) {
        if entry.file_name().and_then(|n| n.to_str()).map(|n| n.contains(".."))
            == Some(true)
        {
            return Err(SecurityError::PathTraversal(entry.display().to_string()));
        }

        if entry.is_symlink() {
            if let Ok(target) = std::fs::read_link(&entry) {
                let resolved = entry.parent().unwrap_or(root).join(&target);
                if let Ok(resolved_canon) = resolved.canonicalize() {
                    if !resolved_canon.starts_with(&root_canon) {
                        return Err(SecurityError::PathTraversal(entry.display().to_string()));
                    }
                }
            }
        }
    }
    Ok(())
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in read.flatten() {
        let path = entry.path();
        out.push(path.clone());
        if path.is_dir() && !path.is_symlink() {
            out.extend(walk(&path));
        }
    }
    out
}
