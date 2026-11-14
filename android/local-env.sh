#!/usr/bin/env sh
# Local Termux build environment; sourced by the repository Gradle Wrapper.
set -e

POCKETTERM_ROOT=${POCKETTERM_ROOT:-$(pwd)}
TOOLS_ROOT="$POCKETTERM_ROOT/.tools"
TERMUX_PREFIX=${PREFIX:-/data/data/com.termux/files/usr}
JAVA_HOME="$TOOLS_ROOT/jdk17"
ANDROID_HOME="$TOOLS_ROOT/android-sdk"
ANDROID_SDK_ROOT="$ANDROID_HOME"
GRADLE_USER_HOME="$TOOLS_ROOT/gradle-home"
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TERMUX_PREFIX/bin/aarch64-linux-android-clang"
AAPT2_BIN="$TOOLS_ROOT/bin/aapt2"
TMPDIR="$TOOLS_ROOT/tmp"

for required in "$JAVA_HOME/bin/java" "$ANDROID_HOME/platforms/android-37.0/android.jar" "$AAPT2_BIN" "$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"; do
    if [ ! -x "$required" ] && [ ! -f "$required" ]; then
        printf 'local-env: missing required local build tool: %s\n' "$required" >&2
        exit 1
    fi
done

mkdir -p "$GRADLE_USER_HOME" "$TMPDIR" "$TOOLS_ROOT/cargo-home"
PATH="$JAVA_HOME/bin:$TOOLS_ROOT/cargo/bin:$TERMUX_PREFIX/bin:$PATH"
LD_LIBRARY_PATH="$JAVA_HOME/lib:$JAVA_HOME/lib/server:$TERMUX_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
CARGO_HOME="$TOOLS_ROOT/cargo-home"
export POCKETTERM_ROOT TOOLS_ROOT TERMUX_PREFIX JAVA_HOME ANDROID_HOME ANDROID_SDK_ROOT
export GRADLE_USER_HOME CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER CARGO_HOME AAPT2_BIN TMPDIR PATH LD_LIBRARY_PATH

cat > "$GRADLE_USER_HOME/gradle.properties" <<PROPERTIES
org.gradle.java.home=$JAVA_HOME
org.gradle.jvmargs=-Xmx1536m -XX:MaxMetaspaceSize=512m -Dfile.encoding=UTF-8 -Djava.io.tmpdir=$TMPDIR
org.gradle.workers.max=2
org.gradle.parallel=false
android.aapt2FromMavenOverride=$AAPT2_BIN
PROPERTIES

cat > "$POCKETTERM_ROOT/android/local.properties" <<PROPERTIES
sdk.dir=$ANDROID_HOME
PROPERTIES
