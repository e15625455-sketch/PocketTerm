#!/usr/bin/env sh
set -eu
PROJECT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if ! command -v gradle >/dev/null 2>&1; then
  echo "build.sh: Gradle 9.7.0 is required (or install the Gradle wrapper)." >&2
  exit 127
fi
exec gradle -p "$PROJECT_DIR" :app:assembleDebug
