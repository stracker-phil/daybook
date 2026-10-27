#!/bin/zsh
# Opens the sample overlay in the running debug app, after a delay so you can switch to another app first.
# Usage: scripts/dev/overlay.sh [sticky] [delay seconds, default 3]
id=sample:open
[[ "$1" == sticky ]] && { id=sample:sticky; shift; }
sleep "${1:-3}"
exec "${0:A:h}/js.sh" "window.__TAURI_INTERNALS__.invoke('run_command', { id: '$id' })"
