// Material's list items as Compose's samples write them (material3 1.5's ListSamples), and the rest of their API as a
// Compose user writes it (ListItemDefaults' colors, shapes, elevation, padding, gap and alignment, ListItemColors'
// properties and state functions, the segmented items). The samples with a RadioButton or a Checkbox in their slots
// (SingleSelection…, MultiSelection…, ListItemWithModeChangeOnLongClick…) come with those controls (WP2), and so does
// SegmentedListItemWithExpansionSample's children's trailing Checkbox (the rest of it here).

@Composable
fun OneLineListItem() {
    Column {
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("One line list item with 24x24 icon") },
            leadingContent = {
                Icon(Icons.Filled.Favorite, contentDescription = "Localized description")
            },
        )
        HorizontalDivider()
    }
}

@Composable
fun TwoLineListItem() {
    Column {
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("Two line list item with trailing") },
            supportingContent = { Text("Secondary text") },
            trailingContent = { Text("meta") },
            leadingContent = {
                Icon(Icons.Filled.Favorite, contentDescription = "Localized description")
            },
        )
        HorizontalDivider()
    }
}

@Composable
fun ThreeLineListItemWithOverlineAndSupporting() {
    Column {
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("Three line list item") },
            overlineContent = { Text("OVERLINE") },
            supportingContent = { Text("Secondary text") },
            leadingContent = {
                Icon(Icons.Filled.Favorite, contentDescription = "Localized description")
            },
            trailingContent = { Text("meta") },
        )
        HorizontalDivider()
    }
}

@Composable
fun ThreeLineListItemWithExtendedSupporting() {
    Column {
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("Three line list item") },
            supportingContent = { Text("Secondary text that\nspans multiple lines") },
            leadingContent = {
                Icon(Icons.Filled.Favorite, contentDescription = "Localized description")
            },
            trailingContent = { Text("meta") },
        )
        HorizontalDivider()
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ClickableListItemSample() {
    Column {
        HorizontalDivider()

        repeat(3) { idx ->
            var count by rememberSaveable { mutableIntStateOf(0) }
            ListItem(
                onClick = { count++ },
                leadingContent = { Icon(Icons.Default.Home, contentDescription = null) },
                trailingContent = { Text("$count") },
                supportingContent = { Text("Additional info") },
                content = { Text("Item ${idx + 1}") },
            )

            HorizontalDivider()
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ClickableListItemWithClickableChildSample() {
    Column {
        HorizontalDivider()

        repeat(3) { idx ->
            ListItem(
                onClick = { /* ListItem onClick callback */ },
                leadingContent = { Icon(Icons.Default.Home, contentDescription = null) },
                trailingContent = {
                    IconButton(onClick = { /* Child onClick callback */ }) {
                        Icon(Icons.Default.Favorite, contentDescription = "Localized description")
                    }
                },
                supportingContent = { Text("The trailing icon has a separate click action") },
                content = { Text("Item ${idx + 1}") },
            )

            HorizontalDivider()
        }
    }
}

// (WP4b: icons) Its children's `trailingContent = { Checkbox(checked = childrenChecked[idx], onCheckedChange = null) }`
// comes with WP2's Checkbox (paste-ready, the sample as written, in the WP4b notes' icons-pending.kt).
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SegmentedListItemWithExpansionSample() {
    var expanded by rememberSaveable { mutableStateOf(false) }
    val numChildren = 3
    val itemCount = 1 + if (expanded) numChildren else 0
    // (Compose's `mutableStateListOf(*Array(numChildren) { false })`: a spread argument comes with stage 14, its three
    // falses written out until then)
    val childrenChecked = rememberSaveable { mutableStateListOf(false, false, false) }

    val colors =
        ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainer)

    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(ListItemDefaults.SegmentedGap),
    ) {
        Spacer(Modifier.height(100.dp))
        SegmentedListItem(
            onClick = { expanded = !expanded },
            modifier =
                Modifier.semantics { stateDescription = if (expanded) "Expanded" else "Collapsed" },
            colors = colors,
            shapes = ListItemDefaults.segmentedShapes(index = 0, count = itemCount),
            leadingContent = { Icon(Icons.Default.Favorite, contentDescription = null) },
            trailingContent = {
                Icon(
                    if (expanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                    contentDescription = null,
                )
            },
            content = { Text("Click to expand/collapse") },
        )
        AnimatedVisibility(
            visible = expanded,
            enter = expandVertically(MaterialTheme.motionScheme.fastSpatialSpec()),
            exit = shrinkVertically(MaterialTheme.motionScheme.fastSpatialSpec()),
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(ListItemDefaults.SegmentedGap)) {
                repeat(numChildren) { idx ->
                    SegmentedListItem(
                        checked = childrenChecked[idx],
                        onCheckedChange = { childrenChecked[idx] = it },
                        colors = colors,
                        shapes =
                            ListItemDefaults.segmentedShapes(index = idx + 1, count = itemCount),
                        leadingContent = {
                            Icon(Icons.Default.Favorite, contentDescription = null)
                        },
                        content = { Text("Child ${idx + 1}") },
                    )
                }
            }
        }
    }
}

// ---- The rest of the API, as a Compose user writes it ----------------------------------------------------------

// ListItemDefaults' colors (by either overload's names), shapes, elevation and padding given to the interactive
// items; ListItemColors read as properties and by state; a segmented list of a single choice, its gap and shapes.
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ListItemForms(log: (String) -> Unit) {
    var plan by remember { mutableStateOf(0) }
    var dark by remember { mutableStateOf(false) }
    val clear = ListItemDefaults.colors(containerColor = Color.Transparent, headlineColor = Color.Red)
    val picked = ListItemDefaults.colors(selectedContainerColor = Color.Green, draggedContainerColor = Color.Blue)
    val segmented = ListItemDefaults.segmentedColors(containerColor = MaterialTheme.colorScheme.surfaceContainer)
    val older =
        ListItemColors(
            containerColor = Color.White,
            headlineColor = Color.Black,
            leadingIconColor = Color.Gray,
            overlineColor = Color.Gray,
            supportingTextColor = Color.Gray,
            trailingIconColor = Color.Gray,
            disabledHeadlineColor = Color.Gray,
            disabledLeadingIconColor = Color.Gray,
            disabledTrailingIconColor = Color.Gray,
        )
    val raised = ListItemElevation(elevation = 1.dp, draggedElevation = 4.dp)
    val round = ListItemDefaults.shapes().copy(pressedShape = CircleShape)
    Column {
        Text("eski: ${older.contentColor == Color.Black && older.selectedContainerColor == Color.Unspecified}")
        Text("kopya: ${clear.copy(contentColor = Color.Blue).contentColor == Color.Blue}")
        Text("yükselti: ${raised.draggedElevation == 4.dp} · biçim: ${round.pressedShape == CircleShape}")
        Text("öntanımlı: ${ListItemDefaults.containerColor == MaterialTheme.colorScheme.surface && ListItemDefaults.contentColor == MaterialTheme.colorScheme.onSurface}")
        Text("saydam: ${clear.containerColor == Color.Transparent} · başlık: ${clear.contentColor == Color.Red}")
        Text("seçili: ${picked.containerColor(enabled = true, selected = true, dragged = false) == Color.Green}")
        Text("sürüklenen: ${picked.containerColor(enabled = true, selected = true, dragged = true) == Color.Blue}")
        Text("boşluk: ${ListItemDefaults.SegmentedGap == 2.dp} · yükselti: ${ListItemDefaults.Elevation == 0.dp}")
        ListItem(
            onClick = { log("tık") },
            onLongClick = { log("uzun") },
            onLongClickLabel = "seçim kipi",
            shapes = ListItemDefaults.shapes(shape = RoundedCornerShape(8.dp)),
            colors = clear,
            elevation = ListItemDefaults.elevation(draggedElevation = 2.dp),
            contentPadding = ListItemDefaults.ContentPadding,
            verticalAlignment = ListItemDefaults.verticalAlignment(),
            modifier = Modifier.testTag("biçimli"),
        ) {
            Text("Biçimli")
        }
        ListItem(checked = dark, onCheckedChange = { dark = it }, colors = picked, elevation = raised, shapes = round) {
            Text("Karanlık: $dark")
        }
        Column(verticalArrangement = Arrangement.spacedBy(ListItemDefaults.SegmentedGap)) {
            for (i in 0 until 3) {
                SegmentedListItem(
                    selected = plan == i,
                    onClick = { plan = i },
                    shapes = ListItemDefaults.segmentedShapes(index = i, count = 3),
                    colors = segmented,
                    modifier = Modifier.testTag("plan $i"),
                    supportingContent = { Text(if (plan == i) "seçili" else "") },
                ) {
                    Text("Plan $i")
                }
            }
        }
        SegmentedListItem(onClick = { log("tek") }, shapes = ListItemDefaults.segmentedShapes(0, 1)) { Text("Tek") }
    }
}
