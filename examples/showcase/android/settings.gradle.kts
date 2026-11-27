// The showcase on Android: the app (`:app`, dev.kompozr.showcase) and KompozR's Android side (`:kompoz`, the Kotlin
// half of kompoz-platform's android module), the Rust library built by cargo-ndk from the app's build.
// Offline: `./gradlew --offline assembleDebug` (or `cargo xtask android`).

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "kompozr-showcase"
include(":app")
include(":kompoz")
project(":kompoz").projectDir = file("../../../crates/kompoz-platform/android")
