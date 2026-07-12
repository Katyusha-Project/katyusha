#ifndef KATYUSHA_CORE_H
#define KATYUSHA_CORE_H

/*
 * Katyusha - C core
 * -----------------
 * Low-level functions used by the Rust layer via FFI.
 * Everything that touches the filesystem and privileges lives here,
 * kept separate from the networking/parsing logic that lives in Rust.
 */

/* Returns 1 if the process is running as root (uid 0), 0 otherwise. */
int k_is_root(void);

/* Creates a directory recursively (equivalent to `mkdir -p`).
 * Returns 0 on success, -1 on error. */
int k_mkdir_p(const char *path);

/* Recursively removes a directory and its contents.
 * Returns 0 on success, -1 on error. */
int k_rm_rf(const char *path);

/* Extracts an archive into dest_dir by invoking the system's `tar`
 * binary in a child process (fork/exec, no popen/system). Compression
 * (gzip, xz, bzip2, or none) is auto-detected by tar itself, so this
 * transparently handles .tar.gz, .tar.xz, .tar.bz2, and plain .tar.
 * Returns 0 on success, -1 on error. */
int k_extract_targz(const char *archive_path, const char *dest_dir);

/* Computes the SHA-256 of a file and writes it as hexadecimal
 * (64 characters + '\0') into out_hex, which must be at least 65 bytes.
 * Returns 0 on success, -1 on error (e.g. file not found). */
int k_sha256_file(const char *path, char *out_hex, unsigned long out_hex_len);

/* Moves a file while preserving permissions (falls back to copy+delete
 * if the rename crosses filesystems). Returns 0 on success, -1 on error. */
int k_move_file(const char *src, const char *dst);

/* Marks a binary as executable (chmod 755). Returns 0 on success, -1 on error. */
int k_make_executable(const char *path);

#endif /* KATYUSHA_CORE_H */
