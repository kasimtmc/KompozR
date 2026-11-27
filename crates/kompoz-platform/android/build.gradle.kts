// KompozR's Android side (kompoz-platform's android module's Kotlin half): the activity and the view an app runs
// in, and as later packages add them the input methods, accessibility, autofill, insets, back and the system's
// services, each a class of its own. An Android library: an app's Gradle project includes it as `:kompoz`
// (examples/showcase/android's settings.gradle.kts) and builds its Rust library beside it with cargo-ndk.

plugins {
    id("com.android.library")
}

android {
    namespace = "dev.kompozr"
    compileSdk {
        version = release(36)
    }

    defaultConfig {
        minSdk = 29
        // The natives are registered by name from the app's library (RegisterNatives): R8 keeps these classes'.
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
