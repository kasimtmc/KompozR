import javax.inject.Inject
import org.gradle.process.ExecOperations

plugins {
    id("com.android.application")
}

// The showcase on Android: KompozActivity (the `:kompoz` module) showing the showcase's library, libshowcase.so —
// examples/showcase's own crate built as a cdylib by cargo-ndk for each ABI (`cargoNdk` below), from KompozR's
// workspace three folders up.
val kompozrWorkspace: File = rootProject.file("../../..").canonicalFile
val rustAbis: List<String> =
    providers.gradleProperty("kompozr.abis").get().split(',').map(String::trim).filter(String::isNotEmpty)
val minSdkLevel = 29
val ndkRelease = "29.0.14206865"

android {
    namespace = "dev.kompozr.showcase"
    compileSdk {
        version = release(36)
    }
    ndkVersion = ndkRelease

    defaultConfig {
        applicationId = "dev.kompozr.showcase"
        minSdk = minSdkLevel
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += rustAbis
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation(project(":kompoz"))
}

/**
 * The app's Rust library for each ABI: `cargo ndk … rustc -p showcase --lib --crate-type cdylib` (the crate is an
 * rlib for the desktop's program and tests; only here a cdylib), each `.so` put where AGP packs it. Cargo knows
 * what is up to date: it runs each build (in a second when nothing changed).
 */
abstract class CargoNdk : DefaultTask() {
    @get:Input abstract val abis: ListProperty<String>
    @get:Input abstract val profile: Property<String>
    @get:Input abstract val workspace: Property<String>
    @get:Input abstract val ndkDirectory: Property<String>
    @get:Input abstract val platform: Property<Int>
    @get:OutputDirectory abstract val output: DirectoryProperty
    @get:Inject abstract val exec: ExecOperations

    @TaskAction
    fun build() {
        val out = output.get().asFile
        out.deleteRecursively()
        val cargo = mutableListOf("ndk")
        abis.get().forEach { cargo += listOf("-t", it) }
        cargo += listOf("--platform", platform.get().toString(), "rustc", "-p", "showcase", "--lib")
        cargo += listOf("--crate-type", "cdylib", "--profile", profile.get())
        exec.exec {
            workingDir = File(workspace.get())
            executable = "cargo"
            args(cargo)
            environment("ANDROID_NDK_HOME", ndkDirectory.get())
        }
        val target = System.getenv("CARGO_TARGET_DIR")?.let(::File) ?: File(workspace.get(), "target")
        val profileDir = if (profile.get() == "dev") "debug" else profile.get()
        val triples = mapOf(
            "arm64-v8a" to "aarch64-linux-android",
            "x86_64" to "x86_64-linux-android",
            "armeabi-v7a" to "armv7-linux-androideabi",
            "x86" to "i686-linux-android",
        )
        for (abi in abis.get()) {
            val triple = triples[abi] ?: throw GradleException("no Rust target for the ABI $abi")
            val built = File(target, "$triple/$profileDir/libshowcase.so")
            if (!built.isFile) throw GradleException("cargo-ndk left no $built")
            built.copyTo(File(out, "$abi/libshowcase.so"), overwrite = true)
        }
    }
}

val cargoNdk = tasks.register<CargoNdk>("cargoNdk") {
    abis.set(rustAbis)
    profile.set(providers.gradleProperty("kompozr.cargoProfile"))
    workspace.set(kompozrWorkspace.path)
    ndkDirectory.set(androidComponents.sdkComponents.sdkDirectory.map { it.dir("ndk/$ndkRelease").asFile.path })
    platform.set(minSdkLevel)
    output.set(layout.buildDirectory.dir("rustJniLibs"))
    outputs.upToDateWhen { false }
}

androidComponents {
    onVariants { variant ->
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoNdk, CargoNdk::output)
    }
}
