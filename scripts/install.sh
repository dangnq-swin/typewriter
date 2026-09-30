#!/bin/sh
# Builds typewriter and installs it for the current user: the `typewriter`
# command, its desktop entry and its icon, and the project file type, so file
# managers open projects in it.
#
#   scripts/install.sh              build and install to ~/.local
#   scripts/install.sh --uninstall  remove what was installed
#
# PREFIX changes where (default ~/.local), e.g.
#   sudo PREFIX=/usr/local scripts/install.sh
# Projects, drafts and settings are never touched.
set -eu

PREFIX=${PREFIX:-"$HOME/.local"}
BIN="$PREFIX/bin/typewriter"
DESKTOP="$PREFIX/share/applications/typewriter.desktop"
ICON="$PREFIX/share/icons/hicolor/scalable/apps/typewriter.svg"
MIME_TYPE=application/x-typewriter-folder
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
    rm -f "$BIN" "$DESKTOP" "$ICON" "$MIME" "$FILE_ICON"
    refresh
    echo "Removed typewriter from $PREFIX."
    exit 0
elif [ $# -gt 0 ]; then
    echo "usage: $0 [--uninstall]" >&2
    exit 2
fi

cd "$(dirname "$0")/.."

cargo build --release --locked -p typewriter-app

install -Dm755 target/release/typewriter "$BIN"
install -Dm644 assets/icons/typewriter.svg "$ICON"
install -Dm644 assets/icons/typewriter.svg "$FILE_ICON"
mkdir -p "$(dirname "$MIME")"
cat >"$MIME" <<MIME
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="$MIME_TYPE">
    <comment>Typewriter folder</comment>
    <sub-class-of type="text/plain"/>
    <icon name="application-x-typewriter-folder"/>
    <glob pattern="*.typr"/>
  </mime-type>
</mime-info>
MIME
mkdir -p "$(dirname "$DESKTOP")"
# Exec: full path, since ~/.local/bin is not always on the desktop's PATH.
# File name = the window's app id, so the desktop pairs them.
cat >"$DESKTOP" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Typewriter
GenericName=Typewriter Simulator
Comment=Write on a simulated mechanical typewriter
Exec="$BIN" %f
Icon=typewriter
Terminal=false
Categories=Office;WordProcessor;
Keywords=typewriter;writing;focus;
MimeType=$MIME_TYPE;
StartupWMClass=typewriter
DESKTOP
refresh

echo "Installed typewriter to $BIN."
case ":$PATH:" in
*":$PREFIX/bin:"*) ;;
*) echo "Add $PREFIX/bin to your PATH to run \`typewriter\` from a terminal." ;;
esac
