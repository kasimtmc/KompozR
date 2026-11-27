// AGP 9.5.1 with its built-in Kotlin, on Kotlin 2.4.21 (the Kotlin Gradle plugin AGP compiles Kotlin with: on the
// build's classpath, AGP's own being older).
buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.21")
    }
}

plugins {
    id("com.android.application") version "9.5.1" apply false
    id("com.android.library") version "9.5.1" apply false
}
