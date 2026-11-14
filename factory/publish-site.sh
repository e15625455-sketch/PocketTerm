#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: publish-site.sh GENERATED_SITE TARGET_REPOSITORY_CHECKOUT" >&2
  exit 2
fi
site=$(realpath "$1")
target=$(realpath "$2")
if [[ ! -f "$site/dists/stable/Release.ed25519" || ! -d "$site/pool/main" || ! -d "$target/.git" ]]; then
  echo "generated signed repository or target checkout is incomplete" >&2
  exit 1
fi

cd "$target"
git fetch origin gh-pages 2>/dev/null || true
if git show-ref --verify --quiet refs/remotes/origin/gh-pages; then
  git checkout -B gh-pages origin/gh-pages
else
  git checkout --orphan gh-pages
  git rm -r --cached --ignore-unmatch . >/dev/null 2>&1 || true
fi
python3 - "$site" "$target" <<'PY'
from pathlib import Path
import shutil, sys
source, target = map(Path, sys.argv[1:])
for name in ("dists", "pool", "licenses", "sources"):
    dest = target / name
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(source / name, dest)
PY

git add -- dists pool licenses sources
if git diff --cached --quiet; then
  echo "package repository is already current"
  exit 0
fi
git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git commit -m "feat: atualizar índice assinado do PocketTerm"
git push origin gh-pages
