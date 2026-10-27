#!/bin/zsh
# Captures the README screenshots from the debug app with a made-up day, never the owner's data.
# Usage: scripts/dev/demo/screens.sh [out-dir]   (default: docs/screens)
# The debug app's data folder is moved aside, filled by prepare.py (generic settings, a demo vault in a temp
# folder, the TimeTracker stand-in) and restored afterwards; DAYBOOK_DEMO_EVENTS supplies the calendar.
set -e
HERE=${0:A:h}
ROOT=${HERE:h:h:h}
DEV="$ROOT/scripts/dev"
BIN="$ROOT/src-tauri/target/debug/bundle/macos/Daybook (Debug).app/Contents/MacOS/daybook"
DATA="$HOME/Library/Application Support/com.philippstracker.daybook.debug"
OUT=${1:-$ROOT/docs/screens}
TMP=$(mktemp -d "${TMPDIR:-/tmp}/daybook-demo.XXXXXX")
[[ -x "$BIN" ]] || { echo "Build first: bun run build:app" >&2; exit 1; }

"$ROOT/scripts/stop-app.sh" "$BIN"
[[ -e "$DATA" ]] && mv "$DATA" "$TMP/data"
restore() {
  "$ROOT/scripts/stop-app.sh" "$BIN"
  rm -rf "$DATA"
  [[ -e "$TMP/data" ]] && mv "$TMP/data" "$DATA"
  rm -rf "$TMP"
  echo "Restored the debug app's data; start it again with scripts/dev/launch.sh"
}
trap restore EXIT

python3 "$HERE/prepare.py" "$DATA" "$TMP/vault"
DAYBOOK_DEMO_EVENTS="$HERE/events.json" "$DEV/launch.sh"
sleep 6
mkdir -p "$OUT"

tab() { "$DEV/js.sh" "(()=>{[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='$1')?.click()})()"; sleep 2; }
overlay() {
  "$BIN" command "$1"
  sleep 1.5
  "$DEV/screenshot.sh" "$OUT/$2.png" --overlay
  "$DEV/js.sh" "void window.__TAURI_INTERNALS__.invoke('panel_close')"
  sleep 0.5
}

"$BIN" show
tab Todos
"$DEV/screenshot.sh" "$OUT/day.png"
tab Activity
"$DEV/screenshot.sh" "$OUT/day-activity.png"
tab Todos
overlay activity:switch activity-overlay
overlay status:toggle status-overlay
overlay journal:open journal-overlay
overlay focus:open focus-overlay
overlay palette:open palette-overlay
ls -1 "$OUT"/*.png
