// Material's rich tooltips as Compose's samples write them: material3 1.5's TooltipSamples.kt rich tooltips as written
// (their @Sampled annotations left out; the plain ones are in snackbars_tooltips.kt, WP5's) — `RichTooltip` is a
// `TooltipScope` extension, called in the tooltip box's `tooltip` lambda, its receiver —, and the rich tooltip's API
// by Kotlin's named arguments.

// ---- TooltipSamples.kt ----

@Composable
fun RichTooltipSample() {
    val tooltipState = rememberTooltipState(isPersistent = true)
    val scope = rememberCoroutineScope()
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = {
            RichTooltip(
                title = { Text(richTooltipSubheadText) },
                action = {
                    TextButton(onClick = { scope.launch { tooltipState.dismiss() } }) {
                        Text(richTooltipActionText)
                    }
                },
            ) {
                Text(richTooltipText)
            }
        },
        hasAction = true,
        state = tooltipState,
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Info, contentDescription = "Localized Description")
        }
    }
}

@Composable
fun RichTooltipWithManualInvocationSample() {
    val tooltipState = rememberTooltipState(isPersistent = true)
    val scope = rememberCoroutineScope()
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        TooltipBox(
            positionProvider =
                TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = {
                RichTooltip(
                    title = { Text(richTooltipSubheadText) },
                    action = {
                        TextButton(onClick = { scope.launch { tooltipState.dismiss() } }) {
                            Text(richTooltipActionText)
                        }
                    },
                ) {
                    Text(richTooltipText)
                }
            },
            hasAction = true,
            state = tooltipState,
        ) {
            IconButton(onClick = { /* Icon button's click event */ }) {
                Icon(imageVector = Icons.Filled.Info, contentDescription = "Localized Description")
            }
        }
        Spacer(Modifier.requiredHeight(30.dp))
        OutlinedButton(onClick = { scope.launch { tooltipState.show() } }) {
            Text("Display tooltip")
        }
    }
}

@Composable
fun RichTooltipWithCaretSample() {
    val tooltipState = rememberTooltipState(isPersistent = true)
    val scope = rememberCoroutineScope()
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = {
            RichTooltip(
                title = { Text(richTooltipSubheadText) },
                action = {
                    TextButton(onClick = { scope.launch { tooltipState.dismiss() } }) {
                        Text(richTooltipActionText)
                    }
                },
                caretShape = TooltipDefaults.caretShape(),
            ) {
                Text(richTooltipText)
            }
        },
        hasAction = true,
        state = tooltipState,
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Info, contentDescription = "Localized Description")
        }
    }
}

@Composable
fun RichTooltipWithCustomCaretSample() {
    val tooltipState = rememberTooltipState(isPersistent = true)
    val scope = rememberCoroutineScope()
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = {
            RichTooltip(
                title = { Text(richTooltipSubheadText) },
                action = {
                    TextButton(onClick = { scope.launch { tooltipState.dismiss() } }) {
                        Text(richTooltipActionText)
                    }
                },
                caretShape = TooltipDefaults.caretShape(DpSize(32.dp, 16.dp)),
            ) {
                Text(richTooltipText)
            }
        },
        hasAction = true,
        state = tooltipState,
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Info, contentDescription = "Localized Description")
        }
    }
}

const val richTooltipSubheadText = "Permissions"
const val richTooltipText =
    "Configure permissions for selected service accounts. " +
        "You can add and remove service account members and assign roles to them. " +
        "Visit go/permissions for details"
const val richTooltipActionText = "Request Access"

// ---- the rich tooltip's API by name ----

// RichTooltip with each parameter by name, RichTooltipColors and its copy, TooltipDefaults' rich members (the
// deprecated rich position provider too); what each gave handed to `seen`.
@Suppress("DEPRECATION")
@Composable
fun RichTooltipsByName(seen: (String) -> Unit) {
    val state = rememberTooltipState(initialIsVisible = true, isPersistent = true)
    val colors = TooltipDefaults.richTooltipColors(
        containerColor = Color.Red,
        contentColor = Color.Blue,
        titleContentColor = Color.Green,
        actionContentColor = Color.Yellow,
    )
    val made = RichTooltipColors(
        containerColor = Color.Red,
        contentColor = Color.Blue,
        titleContentColor = Color.Green,
        actionContentColor = Color.Yellow,
    )
    val copied = colors.copy(containerColor = Color.Gray, contentColor = Color.Unspecified)
    val themed = TooltipDefaults.richTooltipColors()
    seen("colors ${made == colors} ${copied.containerColor == Color.Gray} ${copied.contentColor == Color.Blue}")
    seen("themed ${themed.containerColor == MaterialTheme.colorScheme.surfaceContainer} ${themed.actionContentColor == MaterialTheme.colorScheme.primary}")
    seen("defaults ${TooltipDefaults.richTooltipMaxWidth} ${TooltipDefaults.richTooltipContainerShape == MaterialTheme.shapes.medium}")
    val provider = TooltipDefaults.rememberRichTooltipPositionProvider(spacingBetweenTooltipAndAnchor = 4.dp)
    Box(Modifier.padding(top = 300.dp, start = 100.dp)) {
        TooltipBox(
            positionProvider = provider,
            tooltip = {
                RichTooltip(
                    modifier = Modifier.testTag("zengin"),
                    title = { Text("Başlık") },
                    action = { TextButton(onClick = { state.dismiss() }) { Text("Tamam") } },
                    caretShape = TooltipDefaults.caretShape(caretSize = DpSize(16.dp, 8.dp)),
                    maxWidth = TooltipDefaults.richTooltipMaxWidth,
                    shape = TooltipDefaults.richTooltipContainerShape,
                    colors = colors,
                    tonalElevation = 0.dp,
                    shadowElevation = 3.dp,
                    text = { Text("Metin") },
                )
            },
            state = state,
            hasAction = true,
        ) {
            Icon(Icons.Filled.Info, contentDescription = "Bilgi")
        }
    }
}
