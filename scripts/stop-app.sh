#!/bin/zsh
# Quits the running Daybook, whichever build it is, through its socket and waits until it is gone.
# `running` lists the app's process ids: any build's binary, without a subcommand (the CLI and MCP have one).
# Usage: scripts/stop-app.sh path/to/daybook
running() { pgrep -f 'Daybook( \(Debug\))?\.app/Contents/MacOS/daybook( --hidden)?$'; }
running >/dev/null || exit 0
"$1" quit >/dev/null 2>&1
for _ in {1..50}; do
  running >/dev/null || exit 0
  sleep 0.1
done
echo "Daybook did not quit; quit it from the menu bar and try again" >&2
exit 1
