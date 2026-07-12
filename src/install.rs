//! Package installation logic: look up the package, make sure it's
//! not already handled by something else on the system, pull it down
//! securely, and drop it into place.

use crate::{ffi, manifest, repo, security, system_check};
use std::path::{Path, PathBuf};
use std::process::Command;

const INSTALL_PREFIX: &str = "/opt/katyusha";
const BIN_LINK_DIR: &str = "/usr/local/bin";

pub fn install(name: &str, force: bool) -> Result<(), String> {
    if !ffi::is_root() {
        return Err("root privileges are required. Use: sudo katyusha -i <package>".into());
    }

    if !force {
        if let Some(existing) = system_check::already_installed(name) {
            println!(
                "[i] '{name}' is already installed on this system: {existing}.\n\
                 [i] Skipping install. Use 'sudo katyusha -i {name} --force' to install \
                 Katyusha's own copy alongside it anyway."
            );
            return Ok(());
        }
    }

    println!("[*] Fetching the package index...");
    let packages = repo::load_packages()?;
    let pkg = repo::find(&packages, name)
        .ok_or_else(|| format!("package '{name}' not found in the index"))?;

    println!("[*] Package: {} ({})", pkg.name, pkg.version);
    if let Some(desc) = &pkg.description {
        println!("    {desc}");
    }

    security::require_https(&pkg.url_pack).map_err(|e| e.to_string())?;
    let extra_trusted = security::load_extra_trusted_hosts();
    if let Err(e) = security::check_trusted_host(&pkg.url_pack, &extra_trusted) {
        return Err(format!(
            "{e}\nIf you trust this source, add the host to \
             ~/.config/katyusha/trusted_hosts.txt and try again."
        ));
    }

    let tmp_dir = format!("/tmp/katyusha-install-{}-{}", pkg.name, std::process::id());
    ffi::mkdir_p(&tmp_dir)?;
    let archive_path = format!("{tmp_dir}/pkg.archive");

    println!("[*] Downloading {}...", pkg.url_pack);
    download(&pkg.url_pack, &archive_path)?;

    let checksum = ffi::sha256_file(&archive_path)?;
    println!("[*] SHA-256: {checksum}");

    // Trust-on-first-use: if we've installed this exact name+version
    // before with a different hash, something changed under us.
    let installed = manifest::load();
    if let Some(existing) = manifest::find(&installed, &pkg.name) {
        if existing.version == pkg.version && existing.sha256 != checksum {
            let _ = ffi::rm_rf(&tmp_dir);
            return Err(format!(
                "{}\nIf you intentionally repackaged '{}' {} with new content, \
                 run 'sudo katyusha -r {}' first to clear the old record, then \
                 reinstall.",
                security::verify_checksum(&checksum, &existing.sha256).unwrap_err(),
                pkg.name,
                pkg.version,
                pkg.name
            ));
        }
    }

    let extract_dir = format!("{tmp_dir}/extracted");
    println!("[*] Extracting package...");
    ffi::extract_targz(&archive_path, &extract_dir)?;
    security::audit_extracted_dir(Path::new(&extract_dir)).map_err(|e| e.to_string())?;

    // Source tarballs (and some prebuilt ones too) tend to wrap
    // everything in a `name-version/` folder instead of putting files
    // at the root. Unwrap that before looking for a binary/install.sh.
    let effective_root = unwrap_single_dir(&extract_dir)?;

    let dest_dir = format!("{INSTALL_PREFIX}/{}", pkg.name);
    if Path::new(&dest_dir).exists() {
        ffi::rm_rf(&dest_dir)?;
    }
    ffi::mkdir_p(&dest_dir)?;

    let installed_bin = match place_files(&effective_root, &dest_dir, &pkg.name) {
        Ok(bin) => bin,
        Err(e) => {
            // A failed install shouldn't leave junk around or show up
            // in the manifest as if it succeeded.
            let _ = ffi::rm_rf(&dest_dir);
            let _ = ffi::rm_rf(&tmp_dir);
            return Err(e);
        }
    };

    if let Some(bin_path) = installed_bin {
        let link_path = format!("{BIN_LINK_DIR}/{}", pkg.name);
        let _ = std::fs::remove_file(&link_path);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&bin_path, &link_path)
                .map_err(|e| format!("could not link the binary: {e}"))?;
        }
        println!("[*] Binary linked at {link_path}");
    }

    manifest::add(manifest::InstalledPackage {
        name: pkg.name.clone(),
        version: pkg.version.clone(),
        sha256: checksum,
        prefix: dest_dir.clone(),
    })?;

    let _ = ffi::rm_rf(&tmp_dir);

    println!("[✓] '{}' {} installed successfully.", pkg.name, pkg.version);
    Ok(())
}

fn download(url: &str, dest: &str) -> Result<(), String> {
    let status = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location", "-o", dest, url])
        .status()
        .map_err(|e| format!("could not run curl: {e}"))?;

    if !status.success() {
        return Err(format!("download of '{url}' failed"));
    }
    Ok(())
}

/// Descends into a single wrapping directory (repeatedly, if nested)
/// so package detection always looks at the real root, not a
/// `name-version/` folder tar happened to create on extraction.
fn unwrap_single_dir(dir: &str) -> Result<String, String> {
    let mut current = dir.to_string();
    loop {
        let entries: Vec<PathBuf> = std::fs::read_dir(&current)
            .map_err(|e| format!("could not read '{current}': {e}"))?
            .flatten()
            .map(|e| e.path())
            .collect();

        if entries.len() == 1 && entries[0].is_dir() && !entries[0].is_symlink() {
            current = entries[0].to_string_lossy().to_string();
        } else {
            break;
        }
    }
    Ok(current)
}

/// Copies the extracted package into dest_dir and figures out what to
/// link into PATH: an `install.sh` at the root takes priority (and is
/// run with dest_dir as its argument), otherwise a file matching the
/// package name, otherwise the sole file at the root if there's only
/// one. Anything else is an error — a package that doesn't resolve to
/// something runnable didn't really install, no matter how cleanly the
/// files copied.
fn place_files(extract_dir: &str, dest_dir: &str, pkg_name: &str) -> Result<Option<String>, String> {
    let status = Command::new("cp")
        .args(["-a", &format!("{extract_dir}/."), dest_dir])
        .status()
        .map_err(|e| format!("could not copy the files: {e}"))?;
    if !status.success() {
        return Err("copying files to the final destination failed".to_string());
    }

    let install_script = Path::new(dest_dir).join("install.sh");
    if install_script.exists() {
        ffi::make_executable(&install_script.to_string_lossy())?;
        println!("[*] Running install.sh...");
        let status = Command::new(&install_script)
            .arg(dest_dir)
            .status()
            .map_err(|e| format!("could not run install.sh: {e}"))?;
        if !status.success() {
            return Err("install.sh exited with an error".to_string());
        }
        return Ok(None);
    }

    let named_bin = Path::new(dest_dir).join(pkg_name);
    if named_bin.is_file() {
        ffi::make_executable(&named_bin.to_string_lossy())?;
        return Ok(Some(named_bin.to_string_lossy().to_string()));
    }

    let entries: Vec<PathBuf> = std::fs::read_dir(dest_dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();

    if entries.len() == 1 {
        ffi::make_executable(&entries[0].to_string_lossy())?;
        return Ok(Some(entries[0].to_string_lossy().to_string()));
    }

    Err(format!(
        "no single binary named '{pkg_name}' and no install.sh were found at the \
         archive root. This package likely needs a build step (e.g. it's a source \
         tarball) — repackage it with just the built binary, or add an install.sh. \
         See README.md's 'Publishing packages' section."
    ))
}
