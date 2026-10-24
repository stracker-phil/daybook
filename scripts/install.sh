#!/bin/zsh
# Installs the release build as the one Daybook of this machine: ~/Applications/Daybook.app links to
# it and the `daybook` command links through that app. A running Daybook (any build) restarts on it,
# its window shown or hidden as before.
# `bun run release` ends here; debug builds are never installed (scripts/dev/launch.sh runs them by path).
set -e
ROOT=$(cd "$(dirname "$0")/.." && pwd)
APP="$ROOT/src-tauri/target/release/bundle/macos/Daybook.app"
[[ -d "$APP" ]] || { echo "Not found: $APP (run 'bun run release')"; exit 1; }
LINK="$HOME/Applications/Daybook.app"
BIN="$LINK/Contents/MacOS/daybook"

running() { pgrep -f 'Daybook( \(Debug\))?\.app/Contents/MacOS/daybook( --hidden)?$' >/dev/null; }
# Re-signing a running app's files can get it killed, so it quits first; a hidden window stays hidden.
if running; then
  WAS_RUNNING=1
  OPEN_ARGS=()
  "$APP/Contents/MacOS/daybook" status --json 2>/dev/null | grep -q '"windowVisible": *false' \
    && OPEN_ARGS=(--args --hidden)
  "$ROOT/scripts/stop-app.sh" "$APP/Contents/MacOS/daybook"
fi
"$ROOT/scripts/sign-app.sh" "$APP" com.philippstracker.daybook

mkdir -p "$HOME/Applications" "$HOME/.local/bin"
ln -sfn "$APP" "$LINK"
ln -sf "$BIN" "$HOME/.local/bin/daybook"
echo "Installed the release build as ~/Applications/Daybook.app and ~/.local/bin/daybook"

[[ -n "$WAS_RUNNING" ]] || exit 0
# Launch Services can refuse right after the old app quit (error -600), so try a few times.
for _ in {1..10}; do
  open -g "$LINK" "${OPEN_ARGS[@]}" 2>/dev/null || true
  sleep 1
  running && { echo "Restarted Daybook on the new release build"; exit 0; }
done
echo "Daybook did not start again; open it from ~/Applications" >&2
exit 1
