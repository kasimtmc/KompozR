// What adaptive-layout's samples (ThreePaneScaffoldSample.kt) take: a `val` given its value in each branch, a
// member extension's overload by its argument (`preferredWidth(Float)` beside `preferredWidth(Dp)`), a string to a
// `&str` parameter (a lambda's parameter too), an `Int` compared with an `Int?` and with a safe call of a type the
// index doesn't know, a function parameter given to a nullable shared closure's parameter, and a list of a sealed
// class's values and its nested subclass's.

@Composable
fun Described(selected: Int?) {
    val title: String
    val description: String
    if (selected == null) {
        title = "No item selected"
        description = "Select an item from the list."
    } else {
        title = "Item $selected"
        description = "This is a description about $title."
    }
    Text(title)
    Text(description)
}

@Composable
fun ThreePaneScaffoldPaneScope.Proportioned(width: Float, content: @Composable () -> Unit) {
    AnimatedPane(modifier = Modifier.preferredWidth(width).preferredHeight(0.5f)) { content() }
}

@Composable
fun Routed(route: String) {
    val holder = rememberSaveableStateHolder()
    holder.SaveableStateProvider(route) { Text(route) }
}

@Composable
fun Compared(items: List<String>, selected: Int?) {
    items.forEachIndexed { index, item -> Text(item, color = if (index == selected) Color.Red else Color.Black) }
}

@Composable
fun Picked(items: List<String>, picked: NavItemData?) {
    items.forEachIndexed { index, item -> Text(item, color = if (index == picked?.index) Color.Red else Color.Black) }
}

@Composable
fun Routes(start: String) {
    val holder = rememberSaveableStateHolder()
    AnimatedContent(targetState = start) { route -> holder.SaveableStateProvider(route) { Text(route) } }
}

@Composable
private fun RadioRow(selected: Boolean, onClick: () -> Unit, text: String) {
    Row(Modifier.selectable(selected = selected, onClick = onClick, role = Role.RadioButton)) {
        RadioButton(selected = selected, onClick = onClick)
        Text(text)
    }
}

private val Anchors =
    listOf(
        PaneExpansionAnchor.Proportion(0f),
        PaneExpansionAnchor.Offset.fromStart(240.dp),
        PaneExpansionAnchor.Proportion(1f),
    )
