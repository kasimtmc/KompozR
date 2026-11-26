// KompozR's settings (Settings, Tools, KompozR): the kompoz-lsp to run, its extra arguments, and the shadow-project
// bridge. Applied, they restart the server in the open projects.
package com.plyg.kompozr

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.service
import com.intellij.openapi.options.BoundConfigurable
import com.intellij.openapi.project.ProjectManager
import com.intellij.platform.lsp.api.LspClientManager
import com.intellij.ui.dsl.builder.COLUMNS_LARGE
import com.intellij.ui.dsl.builder.bindSelected
import com.intellij.ui.dsl.builder.bindText
import com.intellij.ui.dsl.builder.columns
import com.intellij.ui.dsl.builder.panel
import com.intellij.util.execution.ParametersListUtil

@Service(Service.Level.APP)
@State(name = "KompozRSettings", storages = [Storage("kompozr.xml")])
class KompozRSettings : PersistentStateComponent<KompozRSettings.Values> {
    class Values {
        var serverPath: String = ""
        var extraArgs: String = ""
        var shadowEnabled: Boolean = true
        var shadowRustAnalyzerPath: String = ""
    }

    private var values = Values()

    override fun getState(): Values = values

    override fun loadState(state: Values) {
        values = state
    }

    var serverPath: String
        get() = values.serverPath
        set(value) {
            values.serverPath = value.trim()
        }

    var extraArgs: String
        get() = values.extraArgs
        set(value) {
            values.extraArgs = value.trim()
        }

    var shadowEnabled: Boolean
        get() = values.shadowEnabled
        set(value) {
            values.shadowEnabled = value
        }

    var shadowRustAnalyzerPath: String
        get() = values.shadowRustAnalyzerPath
        set(value) {
            values.shadowRustAnalyzerPath = value.trim()
        }

    /** The extra arguments as a command line's (quotes kept together). */
    fun extraArguments(): List<String> = ParametersListUtil.parse(extraArgs)

    companion object {
        fun getInstance(): KompozRSettings = service()
    }
}

class KompozRConfigurable : BoundConfigurable("KompozR") {
    override fun createPanel() = panel {
        val settings = KompozRSettings.getInstance()
        row("kompoz-lsp:") {
            textField()
                .bindText(settings::serverPath)
                .columns(COLUMNS_LARGE)
                .comment(
                    "Empty: KOMPOZ_LSP from the environment, else target/debug or target/release/kompoz-lsp in the project " +
                        "(the newest), else kompoz-lsp on PATH or in ~/.cargo/bin.",
                )
        }
        row("Extra arguments:") {
            expandableTextField()
                .bindText(settings::extraArgs)
                .columns(COLUMNS_LARGE)
                .comment("Given to kompoz-lsp after its own (--lsp --no-rust-analyzer: the IDE has its own Rust).")
        }
        group("Shadow-Project Bridge") {
            row {
                checkBox("Enabled")
                    .bindSelected(settings::shadowEnabled)
                    .comment(
                        "A rust-analyzer over the Rust the Kotlin translates to, started when first needed: rustc's type " +
                            "errors and type-accurate hovers on the Kotlin's lines. Off: --no-shadow.",
                    )
            }
            row("rust-analyzer:") {
                textField()
                    .bindText(settings::shadowRustAnalyzerPath)
                    .columns(COLUMNS_LARGE)
                    .comment("The bridge's rust-analyzer (--shadow-rust-analyzer). Empty: kompoz-lsp's own choice, rust-analyzer on PATH.")
            }
        }
    }

    override fun apply() {
        super.apply()
        for (project in ProjectManager.getInstance().openProjects) {
            if (!project.isDisposed) LspClientManager.getInstance(project).stopAndRestartClientsIfNeeded(KompozRLspIntegrationProvider::class.java)
        }
    }
}
