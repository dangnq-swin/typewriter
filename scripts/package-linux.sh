#!/bin/sh
# Builds a Linux release into dist/: a tarball that installs with its own
# install.sh, and an AppImage when appimagetool is given.
#
#   scripts/package-linux.sh 0.1.0
#   APPIMAGETOOL=path/to/appimagetool scripts/package-linux.sh 0.1.0
#   LIBC=musl scripts/package-linux.sh 0.1.0
#
# Build a glibc release on an old distribution: the programs need its glibc
# or newer. LIBC=musl names a build made on a musl system, linked to its
# libraries (a static build can't load the graphics and window libraries).
set -eu

if [ $# -ne 1 ]; then
    echo "usage: $0 version" >&2
    exit 2
fi
version=$1
libc=${LIBC:-gnu}
cd "$(dirname "$0")/.."

cargo build --release --locked -p typewriter-app --bins
dist=dist
rm -rf "$dist"
mkdir -p "$dist"

name="typewriter-$version-x86_64-linux-$libc"
tree="$dist/$name"
mkdir -p "$tree"
install -m755 target/release/typewriter scripts/install.sh "$tree/"
install -m644 assets/icons/typewriter.svg packaging/linux/typewriter.desktop \
    packaging/linux/typewriter.xml LICENSE README.md "$tree/"
tar -C "$dist" -czf "$dist/$name.tar.gz" "$name"
rm -rf "$tree"
echo "$dist/$name.tar.gz"

if [ -z "${APPIMAGETOOL:-}" ]; then
    echo "No APPIMAGETOOL given: no AppImage."
    exit 0
fi
app="$dist/AppDir"
install -Dm755 target/release/typewriter "$app/usr/bin/typewriter"
install -Dm644 assets/icons/typewriter.svg "$app/usr/share/icons/hicolor/scalable/apps/typewriter.svg"
install -Dm644 packaging/linux/typewriter.xml "$app/usr/share/mime/packages/typewriter.xml"
install -m644 packaging/linux/typewriter.desktop assets/icons/typewriter.svg "$app/"
ln -s usr/bin/typewriter "$app/AppRun"
ln -s typewriter.svg "$app/.DirIcon"
ARCH=x86_64 VERSION="$version" "$APPIMAGETOOL" "$app" "$dist/Typewriter-$version-x86_64.AppImage"
rm -rf "$app"
echo "$dist/Typewriter-$version-x86_64.AppImage"
