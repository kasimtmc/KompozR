// Material's button groups as Compose writes them: ButtonGroup's content a lambda of its scope — the deprecated
// overload's composed (`Modifier.animateWidth(source)`, `Modifier.weight(1f)` its member extensions, before
// foundation's `Modifier.weight` of a Row), the items overload's a builder (`clickableItem`, `toggleableItem`,
// `customItem` with the implicit receiver, a composable lambda kept past the content reaching the scope's
// `Modifier.animateWidth`); the overflow indicator a lambda of the menu state (`it`, `menuState.isShowing`);
// `x as RoundedCornerShape` / `as?` (Kotlin's casts through kt's KtAs); `if (b) FontWeight.Bold else null` to a
// nullable parameter.
@Composable
fun Groups(source: MutableInteractionSource, bold: Boolean) {
    var last by remember { mutableStateOf("") }
    ButtonGroup {
        Button(onClick = { last = "oynat" }, modifier = Modifier.animateWidth(source).weight(1f), interactionSource = source) { Text("Oynat") }
    }
    ButtonGroup(
        overflowIndicator = { menuState ->
            IconButton(onClick = { if (menuState.isShowing) menuState.dismiss() else menuState.show() }) {
                Icon(Icons.Filled.MoreVert, contentDescription = "Daha")
            }
        },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        clickableItem(onClick = { last = "kopyala" }, label = "Kopyala")
        toggleableItem(checked = bold, onCheckedChange = {}, label = "Kalın", weight = 2f)
        customItem(
            buttonGroupContent = {
                val held = remember { MutableInteractionSource() }
                Button(onClick = {}, modifier = Modifier.animateWidth(held).align(Alignment.Bottom), interactionSource = held) { Text("Özel") }
            },
            menuContent = { state -> DropdownMenuItem(text = { Text("Özel") }, onClick = { state.dismiss() }) },
        )
    }
    ButtonGroup(overflowIndicator = { ButtonGroupDefaults.OverflowIndicator(it) }) {
        clickableItem(onClick = {}, label = "Bir", icon = { Icon(Icons.Filled.Add, contentDescription = null) })
    }
    Text(last, fontWeight = if (bold) FontWeight.Bold else null)
}

@Composable
fun Casts() {
    val shape = ButtonGroupDefaults.connectedMiddleButtonShapes().shape
    val top = (shape as RoundedCornerShape).copy(topStart = CornerSize(100), topEnd = CornerSize(100))
    val maybe = shape as? CutCornerShape
    Box(Modifier.size(10.dp).background(Color.Red, if (maybe == null) top else shape))
}
