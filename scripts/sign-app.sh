#!/bin/zsh
# Signs a built Daybook bundle so macOS keeps its privacy grants across rebuilds: the grant belongs
# to the identifier and the signing certificate, not to the build. Uses $DAYBOOK_SIGN_IDENTITY, else the
# "Daybook Local Signing" certificate of scripts/create-signing-cert.sh, else signs ad hoc (the grant
# then lasts one build). Usage: scripts/sign-app.sh path/to/App.app bundle.identifier
set -e
APP=$1 IDENTIFIER=$2
IDENTITY=${DAYBOOK_SIGN_IDENTITY:-}
if [[ -z "$IDENTITY" ]] && security find-identity -p codesigning | grep -q '"Daybook Local Signing"'; then
  IDENTITY="Daybook Local Signing"
fi
codesign --force --sign "${IDENTITY:--}" --identifier "$IDENTIFIER" "$APP" 2>/dev/null
[[ -n "$IDENTITY" ]] || echo "Signed ad hoc: grant Accessibility again after each rebuild (scripts/create-signing-cert.sh fixes that)"
