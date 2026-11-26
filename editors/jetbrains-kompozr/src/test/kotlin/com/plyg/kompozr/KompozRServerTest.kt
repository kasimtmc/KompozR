package com.plyg.kompozr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class KompozRServerTest {
    @get:Rule
    val temp = TemporaryFolder()

    private fun built(file: File, millis: Long): File {
        file.parentFile.mkdirs()
        file.writeText("#!/bin/sh\n")
        file.setExecutable(true)
        file.setLastModified(millis)
        return file
    }

    @Test
    fun `the server runs without rust-analyzer, with the shadow bridge's or none, then the extra arguments`() {
        assertEquals(listOf("--lsp", "--no-rust-analyzer"), KompozRServer.arguments(true, "", emptyList()))
        assertEquals(
            listOf("--lsp", "--no-rust-analyzer", "--shadow-rust-analyzer", "/ra"),
            KompozRServer.arguments(true, " /ra ", emptyList()),
        )
        assertEquals(listOf("--lsp", "--no-rust-analyzer", "--no-shadow"), KompozRServer.arguments(false, "/ra", emptyList()))
        assertEquals(
            listOf("--lsp", "--no-rust-analyzer", "--log-file", "/tmp/a b.log"),
            KompozRServer.arguments(true, "", listOf("--log-file", "/tmp/a b.log")),
        )
    }

    @Test
    fun `Rust sources with kompoz are told from plain Rust`() {
        assertTrue(KompozRServer.hasKompoz("kompoz! {\n    fun A() {}\n}\n"))
        assertTrue(KompozRServer.hasKompoz("let p = kompoz![Text(\"a\")];"))
        assertTrue(KompozRServer.hasKompoz("kompoz::kompoz_file!(\"src/main.kt\");"))
        assertFalse(KompozRServer.hasKompoz("fn main() { println!(\"kompoz\"); }"))
        assertFalse(KompozRServer.hasKompoz("my_kompoz! {}"))
    }

    @Test
    fun `kompoz-lsp is the setting's, else KOMPOZ_LSP's, else the newest built, else on PATH, else Cargo's`() {
        val project = temp.newFolder("project")
        val home = temp.newFolder("home")
        val env = mapOf("HOME" to home.path, "PATH" to File(temp.root, "none").path)
        assertEquals("kompoz-lsp", KompozRServer.find("", env, project, false))
        assertEquals("kompoz-lsp.exe", KompozRServer.find("", emptyMap(), project, true))
        val cargo = built(File(home, ".cargo/bin/kompoz-lsp"), 1_000_000)
        assertEquals(cargo.path, KompozRServer.find("", env, project, false))
        val bin = File(temp.root, "bin")
        val onPath = built(File(bin, "kompoz-lsp"), 1_000_000)
        val withPath = env + ("PATH" to "${File(temp.root, "none").path}:${bin.path}")
        assertEquals(onPath.path, KompozRServer.find("", withPath, project, false))
        val debug = built(File(project, "target/debug/kompoz-lsp"), 2_000_000)
        assertEquals(debug.path, KompozRServer.find("", withPath, project, false))
        val release = built(File(project, "target/release/kompoz-lsp"), 3_000_000)
        assertEquals(release.path, KompozRServer.find("", env, project, false))
        val shared = built(File(temp.root, "shared/debug/kompoz-lsp"), 4_000_000)
        assertEquals(shared.path, KompozRServer.find("", env + ("CARGO_TARGET_DIR" to File(temp.root, "shared").path), project, false))
        assertEquals("/opt/kompoz-lsp", KompozRServer.find("", env + ("KOMPOZ_LSP" to "/opt/kompoz-lsp"), project, false))
        assertEquals("/x/kompoz-lsp", KompozRServer.find(" /x/kompoz-lsp ", env + ("KOMPOZ_LSP" to "/opt/kompoz-lsp"), project, false))
    }

    @Test
    fun `a command is found on PATH when it is an executable file there`() {
        val bin = temp.newFolder("bin")
        val ra = built(File(bin, "rust-analyzer"), 1_000_000)
        File(bin, "plain").writeText("")
        assertEquals(ra.path, KompozRServer.onPath("rust-analyzer", mapOf("PATH" to "/nonexistent:${bin.path}"), false))
        assertEquals(null, KompozRServer.onPath("plain", mapOf("PATH" to bin.path), false))
        assertEquals(null, KompozRServer.onPath("rust-analyzer", emptyMap(), false))
    }
}
