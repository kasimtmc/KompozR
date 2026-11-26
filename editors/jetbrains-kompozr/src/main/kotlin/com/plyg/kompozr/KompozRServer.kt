// kompoz-lsp, the server the plugin starts: where it is, the arguments it is given, and which files it is for. Plain
// Kotlin (no IntelliJ classes), for the unit tests; KompozRLsp.kt and KompozRSettings.kt pass the IDE's state in.
package com.plyg.kompozr

import java.io.File

object KompozRServer {
    private val kompoz = Regex("""\bkompoz(?:_file)?!""")

    /** Whether a Rust source has KompozR's Kotlin in it (a kompoz! block, a kompoz_file!). */
    fun hasKompoz(text: CharSequence): Boolean = kompoz.containsMatchIn(text)

    /**
     * kompoz-lsp's arguments: --lsp, and --no-rust-analyzer (the IDE has its own Rust); the shadow-project bridge's
     * rust-analyzer (--shadow-rust-analyzer PATH, when one is set) or no bridge (--no-shadow); then the extra
     * arguments.
     */
    fun arguments(shadow: Boolean, shadowRustAnalyzer: String, extraArgs: List<String>): List<String> = buildList {
        add("--lsp")
        add("--no-rust-analyzer")
        if (!shadow) add("--no-shadow")
        else if (shadowRustAnalyzer.isNotBlank()) addAll(listOf("--shadow-rust-analyzer", shadowRustAnalyzer.trim()))
        addAll(extraArgs)
    }

    /**
     * Where kompoz-lsp is: the setting; else KOMPOZ_LSP from the environment; else one built in the project's target
     * directory (target/debug or target/release, and CARGO_TARGET_DIR's: the newest); else kompoz-lsp on PATH; else
     * Cargo's bin directory (~/.cargo/bin: an IDE started from a desktop may not have it on PATH); else kompoz-lsp,
     * as it is.
     */
    fun find(setting: String, env: Map<String, String>, projectDir: File?, windows: Boolean): String {
        setting.trim().takeIf { it.isNotEmpty() }?.let { return it }
        env["KOMPOZ_LSP"]?.trim()?.takeIf { it.isNotEmpty() }?.let { return it }
        val name = if (windows) "kompoz-lsp.exe" else "kompoz-lsp"
        val targets = mutableListOf<File>()
        projectDir?.let { targets.add(File(it, "target")) }
        env["CARGO_TARGET_DIR"]?.trim()?.takeIf { it.isNotEmpty() }?.let { dir ->
            val file = File(dir)
            if (file.isAbsolute) targets.add(file) else projectDir?.let { targets.add(File(it, dir)) }
        }
        val built = targets.flatMap { target -> listOf("debug", "release").map { File(File(target, it), name) } }
            .filter { it.isFile && it.canExecute() }
            .maxByOrNull { it.lastModified() }
        if (built != null) return built.path
        onPath(name, env, windows)?.let { return it }
        val cargoHome = env["CARGO_HOME"]?.takeIf { it.isNotBlank() }?.let(::File)
            ?: env[if (windows) "USERPROFILE" else "HOME"]?.takeIf { it.isNotBlank() }?.let { File(it, ".cargo") }
        val cargoBin = cargoHome?.let { File(File(it, "bin"), name) }
        if (cargoBin != null && cargoBin.isFile && cargoBin.canExecute()) return cargoBin.path
        return name
    }

    /** The file a command of this name (a file name: kompoz-lsp.exe on Windows) runs from PATH, if there is one. */
    fun onPath(name: String, env: Map<String, String>, windows: Boolean): String? {
        val path = env["PATH"] ?: env["Path"] ?: return null
        return path.split(if (windows) ';' else ':')
            .filter { it.isNotBlank() }
            .map { File(it, name) }
            .firstOrNull { it.isFile && it.canExecute() }
            ?.path
    }
}
