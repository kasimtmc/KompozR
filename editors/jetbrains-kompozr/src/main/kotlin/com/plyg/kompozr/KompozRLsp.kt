// kompoz-lsp, KompozR's language server, in RustRover and CLion: started for .kt files and for Rust sources with
// kompoz! (or kompoz_file!) in them, with --no-rust-analyzer: the IDE has its own Rust. It answers for the Kotlin:
// the generator's errors on the Kotlin's lines (rustc's too, through its shadow-project bridge), hovers, completion,
// going to definitions, and its colors (semantic tokens) for the Kotlin, which the IDE asks for in these files.
package com.plyg.kompozr

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.diagnostic.logger
import com.intellij.openapi.editor.DefaultLanguageHighlighterColors
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.SystemInfo
import com.intellij.openapi.vfs.VfsUtilCore
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspIntegrationProvider
import com.intellij.platform.lsp.api.ProjectWideLspClientDescriptor
import com.intellij.platform.lsp.api.customization.LspCustomization
import com.intellij.platform.lsp.api.customization.LspSemanticTokensSupport
import com.intellij.psi.PsiFile
import java.io.File
import java.util.concurrent.ConcurrentHashMap

private val LOG = logger<KompozRLspIntegrationProvider>()

/** Which files are kompoz-lsp's: .kt files, and Rust sources with kompoz! in them. */
object KompozRSources {
    private val known = ConcurrentHashMap<String, Pair<Long, Boolean>>()  // by path: the text's stamp, the answer

    fun isKotlin(file: VirtualFile): Boolean = file.extension?.lowercase() == "kt"

    fun isKompozR(file: VirtualFile): Boolean {
        if (isKotlin(file)) return true
        if (file.extension?.lowercase() != "rs") return false
        val document = FileDocumentManager.getInstance().getCachedDocument(file)
        val stamp = document?.modificationStamp ?: file.modificationStamp
        known[file.path]?.let { (at, answer) -> if (at == stamp) return answer }
        val text = document?.charsSequence ?: runCatching { VfsUtilCore.loadText(file) }.getOrNull() ?: return false
        return KompozRServer.hasKompoz(text).also { known[file.path] = stamp to it }
    }
}

class KompozRLspIntegrationProvider : LspIntegrationProvider {
    override fun fileOpened(project: Project, file: VirtualFile, clientStarter: LspIntegrationProvider.LspClientStarter) {
        if (!KompozRSources.isKompozR(file)) return
        LOG.info("KompozR: ${file.path}: kompoz-lsp for it")
        clientStarter.ensureClientStarted(KompozRLspClientDescriptor(project))
    }
}

private class KompozRLspClientDescriptor(project: Project) : ProjectWideLspClientDescriptor(project, "KompozR") {
    override fun isSupportedFile(file: VirtualFile): Boolean = KompozRSources.isKompozR(file)

    override fun createCommandLine(): GeneralCommandLine {
        val settings = KompozRSettings.getInstance()
        val server = KompozRServer.find(settings.serverPath, System.getenv(), project.basePath?.let(::File), SystemInfo.isWindows)
        val args = KompozRServer.arguments(settings.shadowEnabled, settings.shadowRustAnalyzerPath, settings.extraArguments())
        LOG.info("KompozR: starting $server ${args.joinToString(" ")}")
        return GeneralCommandLine(listOf(server) + args).withWorkDirectory(project.basePath)
    }

    // (by the file's name: in these IDEs a .kt file is a TextMate one, not Kotlin's)
    override fun getLanguageId(file: VirtualFile): String = if (KompozRSources.isKotlin(file)) "kotlin" else "rust"

    // The server's colors for the Kotlin (its semantic tokens; none for the Rust, which is the IDE's), asked for in
    // the files it serves. An annotation (@Composable) as Kotlin colors one.
    override val lspCustomization = object : LspCustomization() {
        override val semanticTokensCustomizer = object : LspSemanticTokensSupport() {
            override fun shouldAskServerForSemanticTokens(psiFile: PsiFile): Boolean =
                psiFile.virtualFile?.let(KompozRSources::isKompozR) == true

            override fun getTextAttributesKey(tokenType: String, modifiers: List<String>): TextAttributesKey? =
                if (tokenType == "decorator") DefaultLanguageHighlighterColors.METADATA else super.getTextAttributesKey(tokenType, modifiers)
        }
    }
}
