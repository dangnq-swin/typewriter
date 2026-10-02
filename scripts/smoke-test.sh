#!/bin/sh
# Starts a built typewriter on a virtual X display (xvfb-run) with fresh
# folders: it must still be running after 10 s, its window open. The window
# must go to that display and not to a session's compositor, so the Wayland
# socket is hidden from the child.
#
#   scripts/smoke-test.sh target/release/typewriter
set -eu

if [ $# -ne 1 ]; then
    echo "usage: $0 path/to/typewriter" >&2
    exit 2
fi
home=$(mktemp -d)
# xvfb-run only sets DISPLAY: with WAYLAND_DISPLAY about, winit prefers the
# session's compositor and the window shows on the writer's screen instead.
unset WAYLAND_DISPLAY
status=0
XDG_DATA_HOME="$home/data" XDG_CONFIG_HOME="$home/config" \
    timeout 10 xvfb-run -a "$1" >"$home/log" 2>&1 || status=$?
cat "$home/log"
rm -rf "$home"
# 124: still running when the time ran out.
if [ "$status" -ne 124 ]; then
    echo "typewriter stopped (exit $status) instead of running" >&2
    exit 1
fi
echo "typewriter ran for 10 s"
