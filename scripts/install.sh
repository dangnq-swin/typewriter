#!/bin/sh
# Installs typewriter for the current user: the `typewriter` command (normal
# mode) and `typewriter-plain` (plain), their desktop entries and icon, and the
# project file type, so file managers open projects in Typewriter. In
# the source tree it builds first; in a release tarball it installs the
# programs beside it.
#
#   install.sh              install to ~/.local
#   install.sh --uninstall  remove what was installed
#
# PREFIX changes where (default ~/.local), e.g.
#   sudo PREFIX=/usr/local scripts/install.sh
# Projects, drafts and settings are never touched.
set -eu

PREFIX=${PREFIX:-"$HOME/.local"}
BIN="$PREFIX/bin/typewriter"
BIN_PLAIN="$PREFIX/bin/typewriter-plain"
DESKTOP="$PREFIX/share/applications/typewriter.desktop"
DESKTOP_PLAIN="$PREFIX/share/applications/typewriter-plain.desktop"
ICON="$PREFIX/share/icons/hicolor/scalable/apps/typewriter.svg"
MIME="$PREFIX/share/mime/packages/typewriter.xml"
# Named after the type: where file managers look for its icon.
FILE_ICON="$PREFIX/share/icons/hicolor/scalable/mimetypes/application-x-typewriter-folder.svg"

# Refresh desktop menus, file types and icons, if the tools exist.
refresh() {
    if command -v update-mime-database >/dev/null 2>&1; then
        update-mime-database "$PREFIX/share/mime" || true
    fi
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q "$PREFIX/share/applications" || true
    fi
    # Only refresh an existing cache: a new one would hide icons others
    # install later without refreshing it.
    if [ -f "$PREFIX/share/icons/hicolor/icon-theme.cache" ] &&
        command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" || true
    fi
}

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$BIN" "$BIN_PLAIN" "$DESKTOP" "$DESKTOP_PLAIN" "$ICON" "$MIME" "$FILE_ICON"
    refresh
    echo "Removed typewriter from $PREFIX."
    exit 0
elif [ $# -gt 0 ]; then
    echo "usage: $0 [--uninstall]" >&2
    exit 2
fi

here=$(cd "$(dirname "$0")" && pwd)
if [ -x "$here/typewriter" ]; then
    # A release tarball: everything lies beside this script.
    normal="$here/typewriter" plain="$here/typewriter-plain" icon="$here/typewriter.svg"
    desktop="$here/typewriter.desktop" plain_desktop="$here/typewriter-plain.desktop"
    mime="$here/typewriter.xml"
else
    cd "$here/.."
    cargo build --release --locked --workspace --bins
    normal=target/release/typewriter plain=target/release/typewriter-plain
    icon=assets/icons/typewriter.svg
    desktop=packaging/linux/typewriter.desktop
    plain_desktop=packaging/linux/typewriter-plain.desktop
    mime=packaging/linux/typewriter.xml
fi

install -Dm755 "$normal" "$BIN"
install -Dm755 "$plain" "$BIN_PLAIN"
install -Dm644 "$icon" "$ICON"
install -Dm644 "$icon" "$FILE_ICON"
install -Dm644 "$mime" "$MIME"
mkdir -p "$(dirname "$DESKTOP")"
# Exec: full path, since ~/.local/bin is not always on the desktop's PATH.
# File name = the window's app id, so the desktop pairs them.
sed "s|^Exec=typewriter |Exec=\"$BIN\" |" "$desktop" >"$DESKTOP"
sed "s|^Exec=typewriter-plain |Exec=\"$BIN_PLAIN\" |" "$plain_desktop" >"$DESKTOP_PLAIN"
refresh

echo "Installed typewriter to $BIN, and typewriter-plain to $BIN_PLAIN."
case ":$PATH:" in
*":$PREFIX/bin:"*) ;;
*) echo "Add $PREFIX/bin to your PATH to run \`typewriter\` from a terminal." ;;
esac
