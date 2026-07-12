//! FFI bindings to the C core (c_src/core.c).
//!
//! The `libc` crate is deliberately avoided: we use the primitive
//! types from `std::os::raw`, which are enough for this interface
//! and keep the project's dependencies to a minimum.

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_ulong};

extern "C" {
    fn k_is_root() -> c_int;
    fn k_mkdir_p(path: *const c_char) -> c_int;
    fn k_rm_rf(path: *const c_char) -> c_int;
    fn k_extract_targz(archive_path: *const c_char, dest_dir: *const c_char) -> c_int;
    fn k_sha256_file(path: *const c_char, out_hex: *mut c_char, out_hex_len: c_ulong) -> c_int;
    fn k_move_file(src: *const c_char, dst: *const c_char) -> c_int;
    fn k_make_executable(path: *const c_char) -> c_int;
}

fn to_cstring(s: &str) -> CString {
    CString::new(s).expect("path must not contain null bytes")
}

/// Is the current process running as root?
pub fn is_root() -> bool {
    unsafe { k_is_root() == 1 }
}

/// Creates a directory recursively (mkdir -p).
pub fn mkdir_p(path: &str) -> Result<(), String> {
    let c_path = to_cstring(path);
    let rc = unsafe { k_mkdir_p(c_path.as_ptr()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("could not create directory '{}'", path))
    }
}

/// Recursively removes a directory and its contents.
pub fn rm_rf(path: &str) -> Result<(), String> {
    let c_path = to_cstring(path);
    let rc = unsafe { k_rm_rf(c_path.as_ptr()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("could not remove '{}'", path))
    }
}

/// Extracts a .tar.gz archive into dest_dir.
pub fn extract_targz(archive_path: &str, dest_dir: &str) -> Result<(), String> {
    let c_archive = to_cstring(archive_path);
    let c_dest = to_cstring(dest_dir);
    let rc = unsafe { k_extract_targz(c_archive.as_ptr(), c_dest.as_ptr()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!(
            "could not extract '{}' into '{}'",
            archive_path, dest_dir
        ))
    }
}

/// Computes the SHA-256 of a file and returns it as a hex string.
pub fn sha256_file(path: &str) -> Result<String, String> {
    let c_path = to_cstring(path);
    let mut buf: Vec<c_char> = vec![0; 65];
    let rc = unsafe { k_sha256_file(c_path.as_ptr(), buf.as_mut_ptr(), buf.len() as c_ulong) };
    if rc != 0 {
        return Err(format!("could not compute the SHA-256 of '{}'", path));
    }
    let bytes: Vec<u8> = buf[..64].iter().map(|&c| c as u8).collect();
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

/// Moves a file (falling back to copy when crossing filesystems).
pub fn move_file(src: &str, dst: &str) -> Result<(), String> {
    let c_src = to_cstring(src);
    let c_dst = to_cstring(dst);
    let rc = unsafe { k_move_file(c_src.as_ptr(), c_dst.as_ptr()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("could not move '{}' to '{}'", src, dst))
    }
}

/// Marks a file as executable (chmod 755).
pub fn make_executable(path: &str) -> Result<(), String> {
    let c_path = to_cstring(path);
    let rc = unsafe { k_make_executable(c_path.as_ptr()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("could not mark '{}' as executable", path))
    }
}
