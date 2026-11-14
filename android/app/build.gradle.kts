plugins { id("com.android.application") }

val workspaceRoot = rootProject.projectDir.parentFile
val termuxPrefix = providers.environmentVariable("TERMUX_PREFIX")
    .orElse(providers.environmentVariable("PREFIX"))
    .orElse("/data/data/com.termux/files/usr")
    .get()
val jniArm64 = layout.projectDirectory.dir("src/main/jniLibs/arm64-v8a").asFile
val rustJni = File(jniArm64, "libpocketterm_jni.so")
val execShim = File(jniArm64, "libpt-exec.so")

android {
    namespace = "com.pocketterm.android"
    compileSdk = 37
    buildToolsVersion = "37.0.0"

    defaultConfig {
        applicationId = "com.pocketterm.android"
        minSdk = 26
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    buildTypes {
        getByName("debug") { isDebuggable = true }
    }

    packaging { jniLibs { useLegacyPackaging = true } }
    sourceSets["main"].jniLibs.srcDir("src/main/jniLibs")
}

tasks.register<Exec>("compileRustJniArm64") {
    group = "build"
    description = "Build the pocketterm-jni JNI library for Android arm64-v8a."
    workingDir(workspaceRoot)
    environment("CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER", "$termuxPrefix/bin/aarch64-linux-android-clang")
    environment("CARGO_HOME", workspaceRoot.resolve(".tools/cargo-home").absolutePath)
    environment("RUSTFLAGS", "-C link-arg=-Wl,-z,max-page-size=16384")
    commandLine("cargo", "build", "--target", "aarch64-linux-android", "-p", "pocketterm-jni", "--release")
    doLast {
        val built = workspaceRoot.resolve("target/aarch64-linux-android/release/libpocketterm_jni.so")
        if (!built.isFile) error("Cargo did not produce ${built.absolutePath}")
        jniArm64.mkdirs()
        built.copyTo(rustJni, overwrite = true)
    }
}

tasks.register<Exec>("compileExecShimArm64") {
    group = "build"
    description = "Build libpt-exec.so for Android arm64-v8a with 16 KB page alignment."
    workingDir(workspaceRoot)
    doFirst { jniArm64.mkdirs() }
    commandLine(
        "$termuxPrefix/bin/aarch64-linux-android-clang",
        "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
        "-Wl,-z,max-page-size=16384",
        "-o", execShim.absolutePath,
        "native/libpt-exec.c", "-ldl"
    )
}

tasks.named("preBuild").configure {
    dependsOn("compileRustJniArm64", "compileExecShimArm64")
}
