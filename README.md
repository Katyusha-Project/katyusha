# Katyusha

Universal package manager for Linux. System core written in **C**
(low-level operations: privileges, filesystem, extraction, SHA-256),
networking, parsing, and security policy layer in **Rust**.

```
sudo katyusha -i nano             # install
sudo katyusha -i nano --force     # install even if already present elsewhere
sudo katyusha -r nano             # remove
katyusha -s editor                # search the index
katyusha -l                       # list installed packages
katyusha --info                   # show detected distro
katyusha --help
```

## Already-installed detection

Before installing anything, Katyusha checks whether the package is
already present on the system through another means: **APT/dpkg**,
**RPM**, **pacman**, **apk**, **Flatpak**, **Snap**, or simply an
executable with the same name already on `PATH`. If it finds a match
(for example, GNU nano installed via `apt install nano`), it reports
where it came from and skips the install instead of fighting the
system's own package manager for ownership of the binary. Pass
`--force` (or `-f`) to `-i` to install Katyusha's own copy alongside
the existing one anyway.

## Architecture

```
Katyusha/
├── Cargo.toml
├── build.rs             # compiles c_src/core.c and links it as a static lib
├── c_src/
│   ├── core.h
│   └── core.c            # privileges, mkdir -p, rm -rf, tar, self-contained SHA-256
├── debian/
│   ├── control.template   # dpkg control file, VERSION/ARCH filled in at build time
│   └── build-deb.sh        # builds dist/katyusha-<version>.deb
├── src/
│   ├── main.rs            # CLI
│   ├── banner.rs           # ASCII art
│   ├── distro.rs            # distro detection via /etc/os-release
│   ├── repo.rs                # downloading and parsing of index.txt
│   ├── security.rs             # https-only, trusted hosts, TOFU checksum, anti path-traversal
│   ├── system_check.rs          # detects packages already installed by other package managers
│   ├── install.rs                # orchestrates installation
│   ├── remove.rs                  # orchestrates removal
│   ├── manifest.rs                 # registry of packages installed by Katyusha
│   └── ffi.rs                       # bindings to c_src/core.c
└── example-repo/
    ├── index.txt                    # example index to upload to GitHub
    └── install-examples/             # install.sh templates for non-trivial packages
```

## Two separate GitHub repositories

Katyusha's package ecosystem, under the `Katyusha-Project`
organization (`github.com/Katyusha-Project`), is split into two repos:

- **`github.com/Katyusha-Project/katyusha-packages-archive`** — holds
  only the `index.txt` text file (the catalog). This is what
  `KATYUSHA_REPO_URL` points to, and it's what Katyusha downloads on
  every `-i` / `-s` call.
- **`github.com/Katyusha-Project/katyusha-packages`** — holds the
  actual archive files, committed directly into the repo (no GitHub
  Releases involved) under:
  ```
  packages/<vendor>/<name>-<version>/<name>-<version>.tar.gz
  ```
  e.g. `packages/gnu/nano-9.0/nano-9.1.tar.gz`.

Keeping them separate means the index stays a tiny, fast, single-file
download, while the (potentially large) binaries live in their own
repo, browsable at `github.com/Katyusha-Project/katyusha-packages/tree/main/packages/...`.

### Getting the raw download URL

`URL_PACK` entries must point to the **raw** file content, not the
`github.com/.../tree/...` page — that's GitHub's HTML file viewer,
downloading it gets you a webpage, not the archive:

```
github.com/Katyusha-Project/katyusha-packages/tree/main/packages/gnu/nano-9.0/nano-9.1.tar.gz
                                     ↓  drop "tree/", switch host
raw.githubusercontent.com/Katyusha-Project/katyusha-packages/main/packages/gnu/nano-9.0/nano-9.1.tar.gz
```

`raw.githubusercontent.com` is already on Katyusha's default trusted
host list, so no extra configuration is needed.

## Index format (`index.txt`)

One line per package:

```
[NAME][VERSION][URL][URL_PACK][DESCRIPTION optional]
```

- `NAME`: name used in `katyusha -i <NAME>`.
- `VERSION`: package version.
- `URL`: project homepage (informational).
- `URL_PACK`: **https** raw download URL for the archive, under
  `raw.githubusercontent.com/Katyusha-Project/katyusha-packages/...`.
  Accepts `.tar.gz`, `.tar.xz`, `.tar.bz2`, or plain `.tar` — tar
  auto-detects the compression.
- `DESCRIPTION`: optional.

The archive must contain, at its root, either a binary with the same
name as `NAME`, or an executable `install.sh` script (receives the
destination directory as `$1`). Packages that need a build step
(interpreters, anything compiled from source with dependent runtime
files) should ship an `install.sh` — see
`example-repo/install-examples/` for a template.

By default Katyusha reads the index from:
```
https://raw.githubusercontent.com/Katyusha-Project/katyusha-packages-archive/main/index.txt
```

Point it at a fork or private mirror with:

```
export KATYUSHA_REPO_URL="https://raw.githubusercontent.com/your-user/your-repo/main/index.txt"
```

## Security

Katyusha is not an antivirus, but it does actively reduce the typical
attack surface of installing binaries from the internet:

1. **HTTPS only.** Any `URL_PACK` that doesn't start with `https://`
   is rejected before any download is attempted.
2. **Trusted hosts.** By default only GitHub domains are trusted
   (`github.com`, `raw.githubusercontent.com`,
   `objects.githubusercontent.com`, `codeload.github.com`). Other
   hosts require the user to explicitly add them in
   `~/.config/katyusha/trusted_hosts.txt`.
3. **TOFU checksum (trust-on-first-use).** The package's SHA-256 is
   computed with the self-contained C implementation and stored in
   the manifest. If the same version is reinstalled and the hash
   changed, Katyusha aborts: the file was tampered with at the source.
4. **Anti path-traversal audit.** After extracting the archive,
   Katyusha checks that no file or symlink tries to escape the
   extraction directory before moving anything into the system.
5. **System-awareness.** Katyusha detects the distribution
   (`/etc/os-release`) and its family (Debian, RedHat, Arch, SUSE,
   Alpine, Gentoo), and checks other package managers (APT, RPM,
   pacman, apk, Flatpak, Snap) plus `PATH` before installing, to avoid
   duplicating something you already have.

## Building from source

Requires Rust (with Cargo) and a C compiler (`cc`/`gcc`), plus `tar`
and `curl` on the system.

```
cargo build --release
sudo install -m 755 target/release/katyusha /usr/local/bin/katyusha
```

The first build needs internet access so Cargo can fetch the
project's only build-dependency (`cc`, used to compile
`c_src/core.c`). The resulting binary itself only needs network
access to install/search packages.

## Installing via `.deb` (Debian/Ubuntu)

Instead of building manually, package Katyusha as a `.deb`:

```
chmod +x debian/build-deb.sh
./debian/build-deb.sh
sudo dpkg -i dist/katyusha-<version>.deb
```

This builds the release binary, drops it at `/usr/bin/katyusha`
(the standard location for a distro package, unlike the
`/usr/local/bin` used above for a manual build), and declares `curl`
and `tar` as dependencies so `dpkg`/`apt` pull them in if missing.
The version and target architecture are read automatically from
`Cargo.toml` and `dpkg --print-architecture`.

## Publishing packages

1. **Packages repo** (`katyusha-packages`): commit the archive under
   `packages/<vendor>/<name>-<version>/<name>-<version>.tar.gz` and
   push to `main`. No release/tag needed.
2. **Index repo** (`katyusha-packages-archive`): add or update the
   matching line in `index.txt` (see `example-repo/index.txt`), with
   `URL_PACK` built as described above, and push to `main`.
