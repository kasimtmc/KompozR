// Compose for desktop's extras as a Compose user writes them, from a Kotlin file of its own (kotlin_desktop.rs reads
// this file through kompoz_file!): scrollbars beside a scrolling column and a lazy list, a drag of the mouse's
// secondary button through Modifier.onDrag, a context menu whose items a ContextMenuDataProvider adds to, and a
// text field with its own context menu.
package com.example.desktop

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.*
import androidx.compose.foundation.text.*
import androidx.compose.foundation.text.input.*
import androidx.compose.runtime.*
import androidx.compose.ui.*

@Composable
fun DesktopForms() {
    val scroll = rememberScrollState()
    val list = rememberLazyListState()
    var dragged by remember { mutableStateOf(0f) }
    var ended by remember { mutableStateOf(false) }
    var chosen by remember { mutableStateOf("yok") }
    val field = rememberTextFieldState("merhaba")
    Column {
        Box(modifier = Modifier.size(300.dp, 200.dp)) {
            Column(modifier = Modifier.fillMaxSize().verticalScroll(scroll)) {
                Box(modifier = Modifier.fillMaxWidth().height(1000.dp))
            }
            VerticalScrollbar(
                adapter = rememberScrollbarAdapter(scroll),
                modifier = Modifier.align(Alignment.CenterEnd).fillMaxHeight().testTag("dikey"),
            )
        }
        Text("kaydırma: ${scroll.value}")
        Box(modifier = Modifier.size(300.dp, 300.dp)) {
            LazyColumn(modifier = Modifier.fillMaxSize(), state = list) {
                items(100) { i -> Text("Satır $i", modifier = Modifier.height(50.dp)) }
            }
            VerticalScrollbar(
                adapter = rememberScrollbarAdapter(list),
                modifier = Modifier.align(Alignment.CenterEnd).fillMaxHeight().testTag("liste"),
            )
        }
        Box(
            modifier = Modifier.size(100.dp, 30.dp).testTag("sürükle").onDrag(
                matcher = PointerMatcher.mouse(PointerButton.Secondary),
                onDragEnd = { ended = true },
            ) { delta -> dragged += delta.x }
        )
        Text("sürüklendi: ${dragged.roundToInt()}${if (ended) " bitti" else ""}")
        ContextMenuDataProvider(items = { listOf(ContextMenuItem("Eklenen") { chosen = "Eklenen" }) }) {
            ContextMenuArea(items = { listOf(ContextMenuItem("Kendi") { chosen = "Kendi" }) }) {
                Box(modifier = Modifier.size(100.dp, 30.dp).testTag("menü"))
            }
        }
        Text("seçilen: $chosen")
        BasicTextField(state = field, modifier = Modifier.width(200.dp).testTag("alan"))
    }
}
