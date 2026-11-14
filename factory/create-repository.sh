#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: create-repository.sh PACKAGE_OUTPUT_DIR SITE_DIR TERMUX_PACKAGES_CHECKOUT SIGNING_KEY_PEM" >&2
  exit 2
fi

package_dir=$(realpath "$1")
site=$(realpath -m "$2")
termux_src=$(realpath "$3")
signing_key=$(realpath "$4")

if [[ ! -d "$package_dir" || ! -d "$termux_src" || ! -f "$signing_key" ]]; then
  echo "package output, termux-packages checkout, or signing key is missing" >&2
  exit 1
fi
mapfile -d '' packages < <(find "$package_dir" -maxdepth 1 -type f -name '*.deb' -print0 | sort -z)
if [[ ${#packages[@]} -eq 0 ]]; then
  echo "no .deb packages were produced" >&2
  exit 1
fi

mkdir -p "$site/dists/stable/main/binary-aarch64" "$site/pool/main" "$site/licenses" "$site/sources/recipes"
python3 - "$site" <<'PY'
from pathlib import Path
import shutil, sys
root = Path(sys.argv[1])
for name in ("dists", "pool", "licenses", "sources"):
    path = root / name
    if path.exists():
        shutil.rmtree(path)
(root / "dists/stable/main/binary-aarch64").mkdir(parents=True)
(root / "pool/main").mkdir(parents=True)
(root / "licenses").mkdir()
(root / "sources/recipes").mkdir(parents=True)
PY

termux_revision=$(git -C "$termux_src" rev-parse HEAD)
package_names=()
for deb in "${packages[@]}"; do
  package_name=$(dpkg-deb -f "$deb" Package)
  package_version=$(dpkg-deb -f "$deb" Version)
  package_arch=$(dpkg-deb -f "$deb" Architecture)
  if [[ "$package_arch" != aarch64 && "$package_arch" != all ]]; then
    echo "refusing package with unexpected architecture: $package_name ($package_arch)" >&2
    exit 1
  fi
  package_names+=("$package_name=$package_version")
  cp "$deb" "$site/pool/main/"

  recipe=$(find "$termux_src/packages" -type f -path "*/$package_name/build.sh" -print -quit)
  if [[ -n "$recipe" ]]; then
    recipe_dir=$(dirname "$recipe")
    mkdir -p "$site/sources/recipes/$package_name"
    cp -a "$recipe_dir/." "$site/sources/recipes/$package_name/"
  fi

  license_stage=$(mktemp -d)
  dpkg-deb -x "$deb" "$license_stage"
  prefix="$license_stage/data/data/com.pocketterm.android/files/usr/share/doc"
  if [[ -d "$prefix" ]]; then
    while IFS= read -r -d '' license_file; do
      relative=${license_file#"$prefix"/}
      destination="$site/licenses/$package_name/$relative"
      mkdir -p "$(dirname "$destination")"
      cp -L "$license_file" "$destination"
    done < <(find "$prefix" -type f \( -name copyright -o -name LICENSE -o -name COPYING -o -name NOTICE \) -print0)
  fi
  python3 - "$license_stage" <<'PY'
from pathlib import Path
import shutil, sys
shutil.rmtree(Path(sys.argv[1]))
PY
done

packages_index="$site/dists/stable/main/binary-aarch64/Packages"
dpkg-scanpackages --multiversion "$site/pool" /dev/null > "$packages_index"
gzip -n -9 -c "$packages_index" > "$packages_index.gz"
if ! rg -q '^Package: python$' "$packages_index"; then
  echo "the generated aarch64 index does not contain the python package" >&2
  exit 1
fi

source_manifest="$site/sources/README.md"
{
  cat <<EOF2
# PocketTerm package sources and licenses

Packages in this repository were built from the public Termux package recipes at commit [$termux_revision](https://github.com/termux/termux-packages/tree/$termux_revision). The matching recipe directories and patches are copied under `sources/recipes/`; upstream source URLs and checksums are recorded in each `build.sh`. Package copyright and license notices found in the Debian payload are available under `licenses/`.

Built package versions:
EOF2
  printf -- '- `%s`\n' "${package_names[@]}"
} > "$source_manifest"
if [[ -f "$termux_src/LICENSE" ]]; then
  mkdir -p "$site/licenses/termux-packages"
  cp "$termux_src/LICENSE" "$site/licenses/termux-packages/LICENSE"
fi

release="$site/dists/stable/Release"
mkdir -p "$(dirname "$release")"
{
  printf 'Origin: PocketTerm\nLabel: PocketTerm\nSuite: stable\nCodename: stable\n'
  printf 'Date: %s\nArchitectures: aarch64 all\nComponents: main\n' "$(date -u '+%a, %d %b %Y %H:%M:%S UTC')"
  printf 'Description: PocketTerm packages built for com.pocketterm.android\nSHA256:\n'
  for relative in main/binary-aarch64/Packages main/binary-aarch64/Packages.gz; do
    file="$site/dists/stable/$relative"
    digest=$(sha256sum "$file" | cut -d ' ' -f 1)
    size=$(wc -c < "$file" | tr -d ' ')
    printf ' %s %s %s\n' "$digest" "$size" "$relative"
  done
} > "$release"
"$(dirname "$0")/sign-release.sh" "$signing_key" "$release" "$site/dists/stable/Release.ed25519"

if [[ $(du -sb "$site" | cut -f1) -gt 900000000 ]]; then
  echo "repository exceeds the 900 MB safety budget for GitHub Pages" >&2
  exit 1
fi
printf 'Repository created: %s (%s packages)\n' "$site" "${#packages[@]}"
