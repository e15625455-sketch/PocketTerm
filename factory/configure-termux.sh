#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: configure-termux.sh TERMUX_PACKAGES_CHECKOUT" >&2
  exit 2
fi

properties=$(realpath "$1/scripts/properties.sh")
if [[ ! -f "$properties" ]]; then
  echo "termux-packages properties.sh not found: $properties" >&2
  exit 1
fi

python3 - "$properties" <<'PY'
from pathlib import Path
import sys
path = Path(sys.argv[1])
text = path.read_text()
source = 'TERMUX_APP__PACKAGE_NAME="com.termux"'
target = 'TERMUX_APP__PACKAGE_NAME="com.pocketterm.android"'
if target in text:
    raise SystemExit(0)
if text.count(source) != 1:
    raise SystemExit("expected exactly one upstream TERMUX_APP__PACKAGE_NAME default")
path.write_text(text.replace(source, target, 1))
PY

# The current upstream properties.sh derives APP__DATA_DIR, ROOTFS, HOME,
# and PREFIX from TERMUX_APP__PACKAGE_NAME plus its files/home/usr subdirs.
termux_root=$(cd "$(dirname "$properties")/.." && pwd)
resolved=$(cd "$termux_root" && bash -c 'source "$1"; printf "%s\n%s\n%s\n%s" "$TERMUX_APP__PACKAGE_NAME" "$TERMUX_APP__DATA_DIR" "$TERMUX__HOME" "$TERMUX__PREFIX"' _ "$properties")
expected=$'com.pocketterm.android\n/data/data/com.pocketterm.android\n/data/data/com.pocketterm.android/files/home\n/data/data/com.pocketterm.android/files/usr'
if [[ "$resolved" != "$expected" ]]; then
  printf 'unexpected Termux build paths:\n%s\n' "$resolved" >&2
  exit 1
fi
printf 'Configured Termux build: %s\n' "$resolved"
