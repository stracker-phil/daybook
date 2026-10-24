#!/bin/zsh
# Captures the window in each scheme of the running debug app (docs/ux.md, Scheme audit), in the current macOS
# mode: the app follows macOS, so switch the system's appearance and run it again for the other mode.
# screenshot-looks.sh <out-dir>  writes <out-dir>/<scheme>-<mode>.png.
# The settings are restored afterwards.
DIR=${1:?usage: screenshot-looks.sh <out-dir>}
HERE=${0:A:h}
MODE=light
[[ $(defaults read -g AppleInterfaceStyle 2>/dev/null) == Dark ]] && MODE=dark
mkdir -p "$DIR"
"$HERE/js.sh" "window.__looksBefore = window.daybook.getState().settings"
for scheme in default light dark; do
  "$HERE/js.sh" "(() => { const s = window.daybook.getState(); void s.saveSettings({ ...s.settings, ${MODE}Scheme: '$scheme' }, { quiet: true }); })()"
  sleep 1
  "$HERE/screenshot.sh" "$DIR/$scheme-$MODE.png"
done
"$HERE/js.sh" "void window.daybook.getState().saveSettings(window.__looksBefore, { quiet: true })"
