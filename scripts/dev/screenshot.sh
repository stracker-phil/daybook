#!/bin/zsh
# Captures the debug app's window, also in the background: screenshot.sh <out.png> [--overlay]
# Picks its largest normal window so sheets (e.g. Screen Time) don't hide the app; the release app and the
# Glaze prototype are never captured. With --overlay it picks the open overlay instead: the visible window on
# the highest level above normal windows.
MODE=${2:-window}
WID=$(swift - "$MODE" <<'SWIFT' 2>/dev/null
import CoreGraphics
let overlay = CommandLine.arguments[1] == "--overlay"
let all = CGWindowListCopyWindowInfo(.optionAll, kCGNullWindowID) as! [[String: Any]]
var best: (id: Int, layer: Int, area: Double) = (0, -1, 0)
for w in all where (w["kCGWindowIsOnscreen"] as? Bool) == true {
  let owner = w["kCGWindowOwnerName"] as? String ?? ""
  let layer = w["kCGWindowLayer"] as? Int ?? 0
  guard owner == "Daybook (Debug)" else { continue }
  if overlay {
    guard layer > 0, (w["kCGWindowAlpha"] as? Double ?? 0) > 0.5 else { continue }
  } else if layer != 0 {
    continue
  }
  let b = w["kCGWindowBounds"] as! [String: Double]
  let area = b["Width"]! * b["Height"]!
  if layer > best.layer || (layer == best.layer && area > best.area) {
    best = (w["kCGWindowNumber"] as! Int, layer, area)
  }
}
print(best.id)
SWIFT
)
[[ -n "$WID" && "$WID" != 0 ]] || { echo "No window of the debug app to capture" >&2; exit 1; }
screencapture -x -o -l "$WID" "$1"
