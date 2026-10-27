#!/bin/zsh
# Opens every overlay of the running debug app one by one through its CLI, captures each and closes it.
# Usage: scripts/dev/smoke.sh [out-dir]   (default: a new temp dir) writes <out-dir>/<view>.png and lists them.
# Only the debug build's binary is used, never the `daybook` on the PATH. Left out because opening them writes
# or switches: `wizard` (a routine adds its items to today's note) and `offer-off` (a context switch).
ROOT=$(cd "${0:A:h}/../.." && pwd)
HERE=${0:A:h}
BIN="$ROOT/src-tauri/target/debug/bundle/macos/Daybook (Debug).app/Contents/MacOS/daybook"
SOCK="$HOME/Library/Application Support/com.philippstracker.daybook.debug/daybook.sock"
[[ -x "$BIN" ]] || { echo "Build first: bun run build:app" >&2; exit 1; }
pgrep -f 'Daybook \(Debug\)\.app/Contents/MacOS/daybook( --hidden)?$' >/dev/null && [[ -S "$SOCK" ]] \
  || { echo "The debug app isn't running; start it with scripts/dev/launch.sh" >&2; exit 1; }
DIR=${1:-$(mktemp -d "${TMPDIR:-/tmp}/daybook-smoke.XXXXXX")}
mkdir -p "$DIR"

close_overlay() { "$HERE/js.sh" "void window.__TAURI_INTERNALS__.invoke('panel_close')"; sleep 0.5; }

OVERLAYS=(
  palette palette:open
  context context:switch
  status status:toggle
  focus focus:open
  journal journal:open
  quicknotes notes:quick
  search search:open
  links links:open
  jira jira:open
  projects projects:open
  music music:open
  activity activity:switch
)
FAILED=()
close_overlay
for view command in "${OVERLAYS[@]}"; do
  if ! "$BIN" command "$command"; then
    FAILED+=("$view ($command refused)")
    continue
  fi
  sleep 1
  "$HERE/screenshot.sh" "$DIR/$view.png" --overlay || FAILED+=("$view (no overlay to capture)")
  close_overlay
done

ls -1 "$DIR"/*.png 2>/dev/null
(( ${#FAILED} == 0 )) && exit 0
print -l "Not captured:" "${FAILED[@]}" >&2
exit 1
