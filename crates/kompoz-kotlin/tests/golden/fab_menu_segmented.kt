// Material's segmented buttons and FAB menu as Compose writes them: `SegmentedButton(…)` inside a
// Single/MultiChoiceSegmentedButtonRow's lambda (its scope's member, the implicit receiver — each row's own overload);
// `FloatingActionButtonMenuItem(…)` inside the menu's; `checkedProgress` read inside a ToggleFloatingActionButton's
// content, and inside a `graphicsLayer { }` block kept past it (a clone of the scope); a pair's parts typed
// (`item.first` of `listOf(Icons.Filled.X to "…")` an ImageVector, picking `Icon(imageVector, …)`); the toggleable
// FAB's defaults by Kotlin's names (`containerColor()` is `container_color`); a key event's
// properties (`it.type`, `it.key`).
@Composable
fun Segments(pick: Int, bold: Boolean) {
    var picked by remember { mutableStateOf(pick) }
    var checked by remember { mutableStateOf(bold) }
    SingleChoiceSegmentedButtonRow {
        for (i in 0 until 2) {
            SegmentedButton(selected = picked == i, onClick = { picked = i }, shape = SegmentedButtonDefaults.itemShape(i, 2)) { Text("$i") }
        }
    }
    MultiChoiceSegmentedButtonRow {
        SegmentedButton(checked = checked, onCheckedChange = { checked = it }, shape = SegmentedButtonDefaults.baseShape) { Text("Kalın") }
    }
}

@Composable
fun Menu(open: Boolean) {
    var expanded by remember { mutableStateOf(open) }
    val items = listOf(Icons.Filled.Add to "Ekle", Icons.Filled.Edit to "Düzenle")
    FloatingActionButtonMenu(
        expanded = expanded,
        button = {
            ToggleFloatingActionButton(
                checked = expanded,
                onCheckedChange = { expanded = it },
                containerColor = ToggleFloatingActionButtonDefaults.containerColor(),
                containerSize = ToggleFloatingActionButtonDefaults.containerSize(80.dp),
            ) {
                val turned = checkedProgress > 0.5f
                Icon(
                    Icons.Filled.Add,
                    contentDescription = if (turned) "Kapat" else "Aç",
                    modifier = Modifier.animateIcon({ checkedProgress }).graphicsLayer { rotationZ = 45 * checkedProgress },
                )
            }
        },
    ) {
        items.forEachIndexed { i, item ->
            FloatingActionButtonMenuItem(
                onClick = { expanded = false },
                icon = { Icon(item.first, contentDescription = null) },
                text = { Text(text = item.second) },
                modifier = Modifier.onKeyEvent { it.type == KeyEventType.KeyDown && it.key == Key.DirectionUp && i == 0 },
            )
        }
    }
}
