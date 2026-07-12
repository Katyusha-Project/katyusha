#!/bin/bash
# Builds dist/katyusha-<version>.deb from the current source tree.
# No debhelper, no cargo-deb — just cargo build + a hand-rolled
# package tree + dpkg-deb, so the whole process stays inspectable.

set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | sed -E 's/version *= *"(.*)"/\1/')
ARCH=$(dpkg --print-architecture 2>/dev/null || echo amd64)
PKGNAME="katyusha-${VERSION}"

echo "==> Building release binary"
cargo build --release

BUILDROOT="$(mktemp -d)"
trap 'rm -rf "$BUILDROOT"' EXIT

echo "==> Assembling package tree"
mkdir -p "$BUILDROOT/DEBIAN" \
         "$BUILDROOT/usr/bin" \
         "$BUILDROOT/usr/share/doc/katyusha"

install -m 755 target/release/katyusha "$BUILDROOT/usr/bin/katyusha"
install -m 644 README.md "$BUILDROOT/usr/share/doc/katyusha/README.md"
install -m 644 LICENSE "$BUILDROOT/usr/share/doc/katyusha/copyright"

sed -e "s/__VERSION__/${VERSION}/" -e "s/__ARCH__/${ARCH}/" \
    debian/control.template > "$BUILDROOT/DEBIAN/control"

mkdir -p dist
dpkg-deb --build --root-owner-group "$BUILDROOT" "dist/${PKGNAME}.deb"

echo "==> Built dist/${PKGNAME}.deb"
