// KompozR's Kotlin grammar for .kt files (the IDEs this plugin is for have no Kotlin of their own: a .kt file is a
// TextMate one). The bundle is a directory in the plugin's, textmate/kotlin (build.gradle.kts puts it there): the
// TextMate plugin reads bundles from directories, not from inside a jar.
package com.plyg.kompozr

import com.intellij.ide.plugins.cl.PluginAwareClassLoader
import org.jetbrains.plugins.textmate.api.TextMateBundleProvider
import org.jetbrains.plugins.textmate.api.TextMateBundleProvider.PluginBundle
import java.nio.file.Files

class KompozRTextMateBundles : TextMateBundleProvider {
    override fun getBundles(): List<PluginBundle> {
        // (the plugin's own descriptor, by the class loader the IDE gave it)
        val plugin = (javaClass.classLoader as? PluginAwareClassLoader)?.pluginDescriptor ?: return emptyList()
        val bundle = plugin.pluginPath.resolve("textmate").resolve("kotlin")
        return if (Files.isDirectory(bundle)) listOf(PluginBundle("Kotlin (KompozR)", bundle)) else emptyList()
    }
}
