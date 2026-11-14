# Android shell

The project targets Android 17 (API 37), uses AGP 9.5.0 and expects Gradle 9.7.0, an Android SDK platform/build tools 37, CMake 3.22.1, and the Android NDK. Build with `./android/build.sh` after installing those tools. Native C code is packaged from the CMake target with 16 KB maximum page alignment and legacy JNI library packaging.

Status on 2026-11-13: an Android 16/API 36 arm64 device is connected, but this environment has no Java, Gradle, Android SDK or NDK. The Android project and build script are unbuilt and untested here. Follow `../docs/DEVICE_VALIDATION.md` after installing a debug APK.
