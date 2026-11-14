plugins { id("com.android.application") }

android {
    namespace = "com.pocketide.pocketterm"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.pocketide.pocketterm"
        minSdk = 26
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }

    externalNativeBuild { cmake { path = file("../../native/CMakeLists.txt"); version = "3.22.1" } }

    packaging { jniLibs { useLegacyPackaging = true } }

    sourceSets["main"].assets.srcDir("../../web")
}
