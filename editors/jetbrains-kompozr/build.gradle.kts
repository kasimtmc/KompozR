// KompozR's plugin for RustRover and CLion. Built against an IDE of yours (-PidePath=/path/to/rustrover or
// /path/to/clion: no download) or against RustRover 2026.3 downloaded:
//
//   JAVA_HOME=/path/to/rustrover/jbr ./gradlew buildPlugin -PidePath=/path/to/rustrover
//
// (Gradle needs a JDK: the IDE's own, its jbr/, will do.) build/distributions/kompozr-jetbrains-*.zip is then
// installed in the IDE from disk (Settings, Plugins).

plugins {
    id("org.jetbrains.kotlin.jvm") version "2.4.21"
    id("org.jetbrains.intellij.platform") version "2.20.1"
}

group = "com.plyg"
version = "0.1.0"

repositories {
    mavenCentral()
    intellijPlatform { defaultRepositories() }
}

dependencies {
    intellijPlatform {
        val idePath = providers.gradleProperty("idePath")
        if (idePath.isPresent) local(idePath.get()) else rustRover("2026.3")
        bundledPlugin("org.jetbrains.plugins.textmate")
    }
    testImplementation("junit:junit:4.13.2")
}

// The version's entry of CHANGELOG.md, its "- " items as a list: the Marketplace's change notes.
val changeNotesHtml = run {
    // (the version a local: the configuration cache keeps the lambda below, which may not reach the script's own values)
    val notesVersion = version.toString()
    providers.fileContents(layout.projectDirectory.file("CHANGELOG.md")).asText.map { text ->
        val entry = text.substringAfter("## $notesVersion\n", "").substringBefore("\n## ")
        Regex("""^- (.*(?:\n  .*)*)""", RegexOption.MULTILINE).findAll(entry).joinToString("", "<ul>", "</ul>") { item ->
            val line = item.groupValues[1].replace(Regex("""\s*\n\s+"""), " ").replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            "<li>" + line.replace(Regex("`([^`]+)`"), "<code>$1</code>") + "</li>"
        }
    }
}

intellijPlatform {
    pluginConfiguration {
        ideaVersion { sinceBuild = "263" }
        changeNotes = changeNotesHtml
    }
    buildSearchableOptions = false
    // (Kotlin only, no GUI Designer forms: nothing to instrument, and no java-compiler-ant-tasks of the IDE's build to
    // fetch for it)
    instrumentCode = false
    // ./gradlew verifyPlugin: JetBrains' Plugin Verifier against the IDE built with, or against RustRover and CLion
    // 2026.3 downloaded.
    pluginVerification {
        ides {
            val idePath = providers.gradleProperty("idePath")
            if (idePath.isPresent) {
                local(file(idePath.get()))
            } else {
                create("RR", "2026.3")
                create("CL", "2026.3")
            }
        }
    }
}

// The TextMate bundle for .kt files, in the plugin's directory (textmate/kotlin: the TextMate plugin reads a bundle
// from a directory, not from inside a jar): its package.json, and the VS Code extension's Kotlin grammar and language
// configuration, one grammar for both plugins.
val textMateBundle = files(
    "src/textmate/package.json",
    "../vscode-kompozr/syntaxes/kotlin.tmLanguage.json",
    "../vscode-kompozr/language-configuration.json",
)

tasks {
    prepareSandbox {
        from(textMateBundle) {
            into(pluginName.map { "$it/textmate/kotlin" })
            eachFile { if (name.endsWith(".tmLanguage.json")) path = path.substringBeforeLast('/') + "/syntaxes/" + name }
        }
    }
}
