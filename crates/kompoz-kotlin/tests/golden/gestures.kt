// Gestures in a Kotlin file of its own (kompoz_file!), as Compose writes them: a 2D draggable caught at once and a
// 2D scrollable of its state's value (kompozable_tests.cpp's DrawingForms), a transformable that pans only once
// zoomed, and a drag and drop source carrying text to a target given from Rust.
package com.example.gestures

import androidx.compose.foundation.*
import androidx.compose.foundation.draganddrop.*
import androidx.compose.foundation.gestures.*
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.*
import androidx.compose.ui.draganddrop.*
import kotlin.math.*

@Composable
fun Pulled() {
    var pulled by remember { mutableStateOf(Offset.Zero) }
    var panned by remember { mutableStateOf(Offset.Zero) }
    val drag = rememberDraggable2DState { pulled = Offset(pulled.x + it.x, pulled.y + it.y) }
    val pan = rememberScrollable2DState { d ->
        panned = Offset(panned.x + min(d.x, 0f), panned.y + max(d.y, 0f))
        Offset(min(d.x, 0f), max(d.y, 0f))
    }
    Column {
        Box(modifier = Modifier.size(100.dp).draggable2D(drag, startDragImmediately = true).testTag("sürüklenir"))
        Box(modifier = Modifier.size(100.dp).scrollable2D(pan).testTag("kayar"))
        Text("çekilen ${pulled.x.roundToInt()} ${pulled.y.roundToInt()} kaydırılan ${panned.x.roundToInt()} ${panned.y.roundToInt()}")
    }
}

@Composable
fun Zoom() {
    var scale by remember { mutableStateOf(1f) }
    var moved by remember { mutableStateOf(0f) }
    val state = rememberTransformableState { _, zoom, pan, _ ->
        scale *= zoom
        moved += pan.y
    }
    Column {
        Box(modifier = Modifier.size(300.dp).transformable(state = state, canPan = { scale > 1f }).testTag("yakın"))
        Text("ölçek ${(scale * 10).roundToInt()} kayma ${moved.roundToInt()}")
    }
}

@Composable
fun Fruit(name: String, target: DragAndDropTarget) {
    Column {
        Box(
            modifier = Modifier.size(80.dp, 40.dp).dragAndDropSource { _ ->
                DragAndDropTransferData(text = name, mimeTypes = listOf("text/plain"))
            }.testTag("meyve")
        )
        Box(modifier = Modifier.size(200.dp).dragAndDropTarget(shouldStartDragAndDrop = { it.hasMimeType("text/plain") }, target = target))
    }
}
