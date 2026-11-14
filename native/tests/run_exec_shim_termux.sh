#!/data/data/com.termux/files/usr/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
termux_clang=${PREFIX:-/data/data/com.termux/files/usr}/bin/clang
linker64=/system/bin/linker64
if [ ! -x "$termux_clang" ] || [ ! -x "$linker64" ]; then
    printf '%s\n' 'exec-shim-test: Termux clang or Android linker64 not found' >&2
    exit 2
fi

test_root=$(mktemp -d "$repo_root/.tools/tmp/pocketterm-shim.XXXXXX")
mkdir -p "$test_root/usr/bin" "$test_root/usr/lib" "$test_root/usr/tmp"
"$termux_clang" -shared -fPIC -Wall -Wextra -Werror \
    -Wl,-z,max-page-size=16384 \
    -o "$test_root/usr/lib/libpt-exec.so" "$repo_root/native/libpt-exec.c" -ldl
"$termux_clang" -fPIE -pie -Wall -Wextra -Werror \
    -Wl,-z,max-page-size=16384 \
    -o "$test_root/usr/bin/pt-exec-shim-harness" \
    "$repo_root/native/tests/exec_shim_harness.c"
cp "$repo_root/native/tests/shebang-smoke.sh" "$test_root/usr/bin/shebang-smoke.sh"
chmod 700 "$test_root/usr/bin/pt-exec-shim-harness" "$test_root/usr/bin/shebang-smoke.sh"

printf 'Termux shim test root: %s\n' "$test_root"
env \
    PREFIX="$test_root/usr" \
    POCKETTERM_FILES_DIR="$test_root" \
    HOME="$test_root" \
    TMPDIR="$test_root/usr/tmp" \
    PATH="$test_root/usr/bin:/system/bin" \
    LD_LIBRARY_PATH="$test_root/usr/lib" \
    LD_PRELOAD="$test_root/usr/lib/libpt-exec.so" \
    "$linker64" "$test_root/usr/bin/pt-exec-shim-harness" \
    "$test_root/usr/bin/pt-exec-shim-harness" \
    "$test_root/usr/bin/shebang-smoke.sh"
