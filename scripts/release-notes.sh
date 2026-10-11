#!/usr/bin/env bash
# Prints the release notes for a version: its CHANGELOG section plus the
# downloads footer. Usage: scripts/release-notes.sh 0.1.0
set -euo pipefail
version="$1"
root="$(cd "$(dirname "$0")/.." && pwd)"

awk -v v="$version" '
  index($0, "## [" v "]") == 1 { found = 1; next }
  found && /^## / { exit }
  found { print }
' "$root/CHANGELOG.md"
sed "s/<version>/$version/g" "$root/docs/release-footer.md"
