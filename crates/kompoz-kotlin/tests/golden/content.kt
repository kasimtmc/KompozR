// Receiving content in a Kotlin file of its own (kompoz_file!), as Compose writes it: receivers inside receivers
// (the nearest first, what it leaves to the one around it), each taking the items it wants with `consume`; and the
// clipboard through LocalClipboard's suspending getClipEntry/setClipEntry from a coroutine, its entry made of text.
package com.example.content

import androidx.compose.foundation.*
import androidx.compose.foundation.content.*
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.*
import androidx.compose.ui.platform.*
import kotlinx.coroutines.launch

@Composable
fun NestedReceivers(log: (String) -> Unit) {
    Box(
        modifier = Modifier.size(200.dp).contentReceiver { content ->
            log("dış ${content.clipEntry.itemCount}")
            content.consume { item -> item.text() != null }
        },
    ) {
        Box(
            modifier = Modifier.size(100.dp).contentReceiver { content ->
                log("iç ${content.clipEntry.itemCount} ${content.hasMediaType(MediaType.Image)}")
                content.consume { item -> item.hasMimeType("image/*") }
            },
        )
    }
}

@Composable
fun ClipboardButtons() {
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    var pasted by remember { mutableStateOf("") }
    Column {
        Text("kopyala", modifier = Modifier.clickable {
            scope.launch { clipboard.setClipEntry(ClipEntry.withPlainText("panodan")) }
        })
        Text("yapıştır", modifier = Modifier.clickable {
            scope.launch { pasted = clipboard.getClipEntry()?.readPlainText() ?: "" }
        })
        Text("pano: $pasted")
    }
}

// (kotlin_content.rs' kompoz! part: a field's receiver returning early with `return@contentReceiver`, and a nullable
// result made of an `if`'s branches — the content as it is, or what `consume` leaves)
@Composable
fun PasteImages(received: (Int, Int) -> Unit) {
    val state = rememberTextFieldState()
    Column {
        BasicTextField(
            state = state,
            modifier = Modifier.width(300.dp).testTag("alan").contentReceiver { transferableContent ->
                if (!transferableContent.hasMediaType(MediaType.Image)) {
                    return@contentReceiver transferableContent
                }
                transferableContent.consume { item ->
                    val image = item.image()
                    if (image != null) {
                        received(image.width, image.height)
                        true
                    } else {
                        false
                    }
                }
            },
        )
        Text("alan: ${state.text}")
    }
}

@Composable
fun DropFiles(dropped: (String) -> Unit) {
    Box(
        modifier = Modifier.size(100.dp).contentReceiver { content ->
            if (content.source != TransferableContent.Source.DragAndDrop) {
                content
            } else {
                content.consume { item ->
                    item.files().forEach { dropped(it) }
                    item.files().isNotEmpty()
                }
            }
        },
    )
}
