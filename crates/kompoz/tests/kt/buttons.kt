// Material's buttons and containers as Compose's samples write them (material3 1.5's ButtonSamples,
// FloatingActionButtonSamples, IconButtonSamples, CardSamples, ChipSamples, BadgeSamples). The samples composing in a
// TooltipBox (WP7), a Scaffold (WP4a) or a NavigationBar (WP4b) come with those packages; ChipGroupReflowSample with
// icons-extended (its Icons.Filled.Tune: WP9). ChipGroupSingleLineSample has its DropdownMenu (WP3's).

@Composable
fun ButtonSample() {
    Button(onClick = {}) { Text("Button") }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ButtonWithAnimatedShapeSample() {
    Button(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Button") }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SquareButtonSample() {
    Button(onClick = { /* Do something! */ }, shape = ButtonDefaults.squareShape) { Text("Button") }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SmallButtonSample() {
    Button(onClick = { /* Do something! */ }, contentPadding = ButtonDefaults.SmallContentPadding) {
        Text("Button")
    }
}

@Composable
fun ElevatedButtonSample() {
    ElevatedButton(onClick = { /* Do something! */ }) { Text("Elevated Button") }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ElevatedButtonWithAnimatedShapeSample() {
    ElevatedButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Elevated Button") }
}

@Composable
fun FilledTonalButtonSample() {
    FilledTonalButton(onClick = { /* Do something! */ }) { Text("Filled Tonal Button") }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun FilledTonalButtonWithAnimatedShapeSample() {
    FilledTonalButton(onClick = {}, shapes = ButtonDefaults.shapes()) {
        Text("Filled Tonal Button")
    }
}

@Composable
fun OutlinedButtonSample() {
    OutlinedButton(onClick = { /* Do something! */ }) { Text("Outlined Button") }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun OutlinedButtonWithAnimatedShapeSample() {
    OutlinedButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Outlined Button") }
}

@Composable
fun TextButtonSample() {
    TextButton(onClick = { /* Do something! */ }) { Text("Text Button") }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun TextButtonWithAnimatedShapeSample() {
    TextButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Text Button") }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ButtonWithIconSample() {
    Button(
        onClick = { /* Do something! */ },
        contentPadding =
            ButtonDefaults.contentPaddingFor(ButtonDefaults.MinHeight, hasStartIcon = true),
    ) {
        Icon(
            Icons.Filled.Favorite,
            contentDescription = "Localized description",
            modifier = Modifier.size(ButtonDefaults.iconSizeFor(ButtonDefaults.MinHeight)),
        )
        Spacer(Modifier.size(ButtonDefaults.iconSpacingFor(ButtonDefaults.MinHeight)))
        Text("Like")
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun XSmallButtonWithIconSample() {
    val size = ButtonDefaults.ExtraSmallContainerHeight
    Button(
        onClick = { /* Do something! */ },
        modifier = Modifier.heightIn(size),
        contentPadding = ButtonDefaults.contentPaddingFor(size, hasStartIcon = true),
    ) {
        Icon(
            Icons.Filled.Edit,
            contentDescription = "Localized description",
            modifier = Modifier.size(ButtonDefaults.iconSizeFor(size)),
        )
        Spacer(Modifier.size(ButtonDefaults.iconSpacingFor(size)))
        Text("Label")
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MediumButtonWithIconSample() {
    val size = ButtonDefaults.MediumContainerHeight
    Button(
        onClick = { /* Do something! */ },
        modifier = Modifier.heightIn(size),
        contentPadding = ButtonDefaults.contentPaddingFor(size, hasStartIcon = true),
    ) {
        Icon(
            Icons.Filled.Edit,
            contentDescription = "Localized description",
            modifier = Modifier.size(ButtonDefaults.iconSizeFor(size)),
        )
        Spacer(Modifier.size(ButtonDefaults.iconSpacingFor(size)))
        Text("Label", style = ButtonDefaults.textStyleFor(size))
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LargeButtonWithIconSample() {
    val size = ButtonDefaults.LargeContainerHeight
    Button(
        onClick = { /* Do something! */ },
        modifier = Modifier.heightIn(size),
        contentPadding = ButtonDefaults.contentPaddingFor(size, hasStartIcon = true),
    ) {
        Icon(
            Icons.Filled.Edit,
            contentDescription = "Localized description",
            modifier = Modifier.size(ButtonDefaults.iconSizeFor(size)),
        )
        Spacer(Modifier.size(ButtonDefaults.iconSpacingFor(size)))
        Text("Label", style = ButtonDefaults.textStyleFor(size))
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun XLargeButtonWithIconSample() {
    val size = ButtonDefaults.ExtraLargeContainerHeight
    Button(
        onClick = { /* Do something! */ },
        modifier = Modifier.heightIn(size),
        contentPadding = ButtonDefaults.contentPaddingFor(size, hasStartIcon = true),
    ) {
        Icon(
            Icons.Filled.Edit,
            contentDescription = "Localized description",
            modifier = Modifier.size(ButtonDefaults.iconSizeFor(size)),
        )
        Spacer(Modifier.size(ButtonDefaults.iconSpacingFor(size)))
        Text("Label", style = ButtonDefaults.textStyleFor(size))
    }
}

@Composable
fun ExtendedFloatingActionButtonTextSample() {
    ExtendedFloatingActionButton(onClick = { /* do something */ }) { Text(text = "Extended FAB") }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SmallExtendedFloatingActionButtonTextSample() {
    SmallExtendedFloatingActionButton(onClick = { /* do something */ }) {
        Text(text = "Small Extended FAB")
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MediumExtendedFloatingActionButtonTextSample() {
    MediumExtendedFloatingActionButton(onClick = { /* do something */ }) {
        Text(text = "Medium Extended FAB")
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LargeExtendedFloatingActionButtonTextSample() {
    LargeExtendedFloatingActionButton(onClick = { /* do something */ }) {
        Text(text = "Large Extended FAB")
    }
}

@Composable
fun ExtendedFloatingActionButtonSample() {
    ExtendedFloatingActionButton(
        onClick = { /* do something */ },
        icon = { Icon(Icons.Filled.Add, "Localized description") },
        text = { Text(text = "Extended FAB") },
    )
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SmallExtendedFloatingActionButtonSample() {
    SmallExtendedFloatingActionButton(
        onClick = { /* do something */ },
        icon = { Icon(Icons.Filled.Add, "Localized description") },
        text = { Text(text = "Small Extended FAB") },
    )
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MediumExtendedFloatingActionButtonSample() {
    MediumExtendedFloatingActionButton(
        onClick = { /* do something */ },
        icon = {
            Icon(
                Icons.Filled.Add,
                "Localized description",
                modifier = Modifier.size(FloatingActionButtonDefaults.MediumIconSize),
            )
        },
        text = { Text(text = "Medium Extended FAB") },
    )
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LargeExtendedFloatingActionButtonSample() {
    LargeExtendedFloatingActionButton(
        onClick = { /* do something */ },
        icon = {
            Icon(
                Icons.Filled.Add,
                "Localized description",
                modifier = Modifier.size(FloatingActionButtonDefaults.LargeIconSize),
            )
        },
        text = { Text(text = "Large Extended FAB") },
    )
}

@Composable
fun CardSample() {
    Card(Modifier.size(width = 180.dp, height = 100.dp)) {
        Box(Modifier.fillMaxSize()) { Text("Card content", Modifier.align(Alignment.Center)) }
    }
}

@Composable
fun ClickableCardSample() {
    Card(
        onClick = { /* Do something */ },
        modifier = Modifier.size(width = 180.dp, height = 100.dp),
    ) {
        Box(Modifier.fillMaxSize()) { Text("Clickable", Modifier.align(Alignment.Center)) }
    }
}

@Composable
fun ElevatedCardSample() {
    ElevatedCard(Modifier.size(width = 180.dp, height = 100.dp)) {
        Box(Modifier.fillMaxSize()) { Text("Card content", Modifier.align(Alignment.Center)) }
    }
}

@Composable
fun ClickableElevatedCardSample() {
    ElevatedCard(
        onClick = { /* Do something */ },
        modifier = Modifier.size(width = 180.dp, height = 100.dp),
    ) {
        Box(Modifier.fillMaxSize()) { Text("Clickable", Modifier.align(Alignment.Center)) }
    }
}

@Composable
fun OutlinedCardSample() {
    OutlinedCard(Modifier.size(width = 180.dp, height = 100.dp)) {
        Box(Modifier.fillMaxSize()) { Text("Card content", Modifier.align(Alignment.Center)) }
    }
}

@Composable
fun ClickableOutlinedCardSample() {
    OutlinedCard(
        onClick = { /* Do something */ },
        modifier = Modifier.size(width = 180.dp, height = 100.dp),
    ) {
        Box(Modifier.fillMaxSize()) { Text("Clickable", Modifier.align(Alignment.Center)) }
    }
}

@Composable
fun AssistChipSample() {
    AssistChip(
        onClick = { /* Do something! */ },
        label = { Text("Assist Chip") },
        leadingIcon = {
            Icon(
                Icons.Filled.Settings,
                contentDescription = "Localized description",
                Modifier.size(AssistChipDefaults.IconSize),
            )
        },
    )
}

@Composable
fun ElevatedAssistChipSample() {
    ElevatedAssistChip(
        onClick = { /* Do something! */ },
        label = { Text("Assist Chip") },
        leadingIcon = {
            Icon(
                Icons.Filled.Settings,
                contentDescription = "Localized description",
                Modifier.size(AssistChipDefaults.IconSize),
            )
        },
    )
}

@Composable
fun FilterChipSample() {
    var selected by remember { mutableStateOf(false) }
    FilterChip(
        selected = selected,
        onClick = { selected = !selected },
        label = { Text("Filter chip") },
        leadingIcon =
            if (selected) {
                {
                    Icon(
                        imageVector = Icons.Filled.Done,
                        contentDescription = "Localized Description",
                        modifier = Modifier.size(FilterChipDefaults.IconSize),
                    )
                }
            } else {
                null
            },
    )
}

@Composable
fun ElevatedFilterChipSample() {
    var selected by remember { mutableStateOf(false) }
    ElevatedFilterChip(
        selected = selected,
        onClick = { selected = !selected },
        label = { Text("Filter chip") },
        leadingIcon =
            if (selected) {
                {
                    Icon(
                        imageVector = Icons.Filled.Done,
                        contentDescription = "Localized Description",
                        modifier = Modifier.size(FilterChipDefaults.IconSize),
                    )
                }
            } else {
                null
            },
    )
}

@Composable
fun FilterChipWithLeadingIconSample() {
    var selected by remember { mutableStateOf(false) }
    FilterChip(
        selected = selected,
        onClick = { selected = !selected },
        label = { Text("Filter chip") },
        leadingIcon =
            if (selected) {
                {
                    Icon(
                        imageVector = Icons.Filled.Done,
                        contentDescription = "Localized Description",
                        modifier = Modifier.size(FilterChipDefaults.IconSize),
                    )
                }
            } else {
                {
                    Icon(
                        imageVector = Icons.Filled.Home,
                        contentDescription = "Localized description",
                        modifier = Modifier.size(FilterChipDefaults.IconSize),
                    )
                }
            },
    )
}

@Composable
fun FilterChipWithTrailingIconSample() {
    var selected by remember { mutableStateOf(false) }
    FilterChip(
        selected = selected,
        onClick = { selected = !selected },
        label = { Text("Filter chip") },
        leadingIcon =
            if (selected) {
                {
                    Icon(
                        imageVector = Icons.Filled.Done,
                        contentDescription = "Localized Description",
                        modifier = Modifier.size(FilterChipDefaults.IconSize),
                    )
                }
            } else {
                null
            },
        trailingIcon = {
            Icon(
                imageVector = Icons.Filled.ArrowDropDown,
                contentDescription = "Localized Description",
                modifier = Modifier.size(FilterChipDefaults.IconSize),
            )
        },
    )
}

@Composable
fun FilterChipWithCustomSpacingSample() {
    var selected by remember { mutableStateOf(false) }
    FilterChip(
        selected = selected,
        onClick = { selected = !selected },
        label = { Text("Filter chip") },
        leadingIcon =
            if (selected) {
                {
                    Icon(
                        imageVector = Icons.Filled.Done,
                        contentDescription = "Localized Description",
                        modifier = Modifier.size(FilterChipDefaults.IconSize),
                    )
                }
            } else {
                null
            },
        horizontalArrangement = FilterChipDefaults.horizontalArrangement(4.dp),
    )
}

@Composable
fun InputChipSample() {
    var selected by remember { mutableStateOf(false) }
    InputChip(
        selected = selected,
        onClick = { selected = !selected },
        label = { Text("Input Chip") },
    )
}

@Composable
fun InputChipWithAvatarSample() {
    var selected by remember { mutableStateOf(false) }
    InputChip(
        selected = selected,
        onClick = { selected = !selected },
        label = { Text("Input Chip") },
        avatar = {
            Icon(
                Icons.Filled.Person,
                contentDescription = "Localized description",
                Modifier.size(InputChipDefaults.AvatarSize),
            )
        },
    )
}

@Composable
fun SuggestionChipSample() {
    SuggestionChip(onClick = { /* Do something! */ }, label = { Text("Suggestion Chip") })
}

@Composable
fun ElevatedSuggestionChipSample() {
    ElevatedSuggestionChip(onClick = { /* Do something! */ }, label = { Text("Suggestion Chip") })
}

// (its Icons.Filled.Tune and Icons.AutoMirrored.Filled.ArrowRight are of the extended icons, WP9's: the core set's
// Settings and KeyboardArrowRight in their places, as kompoz-material3's tests/containers.rs has them)
@Composable
fun ChipGroupSingleLineSample() {
    var expanded by remember { mutableStateOf(false) }
    val listSize = 9
    val chipData = List(listSize) { index -> "Chip $index" }
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Row(modifier = Modifier.horizontalScroll(rememberScrollState()).testTag("ChipGroupSingleLineSample")) {
            AssistChip(
                onClick = { expanded = !expanded },
                label = { Text("Show All") },
                leadingIcon = {
                    Icon(
                        imageVector = Icons.Filled.Settings,
                        contentDescription = "Localized Description",
                        modifier = Modifier.size(FilterChipDefaults.IconSize),
                    )
                },
            )
            /*
             * When chip lists exceed the available horizontal screen space, one option is to
             * provide a chip button that opens a menu displaying all chip options. This ensures
             * all options are accessible while maintaining the position of the content below the
             * chip list.
             */
            DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                repeat(listSize) { index ->
                    DropdownMenuItem(
                        text = { Text(chipData[index]) },
                        onClick = {},
                        trailingIcon = {
                            Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null)
                        },
                    )
                }
            }
            repeat(listSize) { index ->
                AssistChip(
                    modifier = Modifier.padding(horizontal = 4.dp),
                    onClick = { /* do something*/ },
                    label = { Text(chipData[index]) },
                    trailingIcon = { Icon(Icons.Filled.ArrowDropDown, contentDescription = null) },
                )
            }
        }
    }
}

// IconButtonSamples and the plain FAB samples of FloatingActionButtonSamples, their TooltipBox (WP7) left out (the
// button it wraps as it is).

@Composable
fun IconButtonSample() {
    val description = "Localized description"
    IconButton(onClick = { /* doSomething() */ }) {
        Icon(Icons.Filled.Lock, contentDescription = description)
    }
}

@Composable
fun IconButtonWithAnimatedShapeSample() {
    val description = "Localized description"
    IconButton(onClick = { /* doSomething() */ }, shapes = IconButtonDefaults.shapes()) {
        Icon(Icons.Filled.Lock, contentDescription = description)
    }
}

@Composable
fun ExtraSmallNarrowSquareIconButtonsSample() {
    val description = "Localized description"
    // Small narrow round icon button
    FilledIconButton(
        onClick = { /* doSomething() */ },
        modifier =
            Modifier.minimumInteractiveComponentSize()
                .size(
                    IconButtonDefaults.extraSmallContainerSize(
                        IconButtonDefaults.IconButtonWidthOption.Narrow
                    )
                ),
        shape = IconButtonDefaults.extraSmallSquareShape,
    ) {
        Icon(
            Icons.Filled.Lock,
            contentDescription = description,
            modifier = Modifier.size(IconButtonDefaults.extraSmallIconSize),
        )
    }
}

@Composable
fun MediumRoundWideIconButtonSample() {
    val description = "Localized description"
    IconButton(
        onClick = { /* doSomething() */ },
        modifier =
            Modifier.size(
                IconButtonDefaults.mediumContainerSize(
                    IconButtonDefaults.IconButtonWidthOption.Wide
                )
            ),
        shape = IconButtonDefaults.mediumRoundShape,
    ) {
        Icon(
            Icons.Filled.Lock,
            contentDescription = description,
            modifier = Modifier.size(IconButtonDefaults.mediumIconSize),
        )
    }
}

@Composable
fun LargeRoundUniformOutlinedIconButtonSample() {
    val description = "Localized description"
    OutlinedIconButton(
        onClick = { /* doSomething() */ },
        modifier = Modifier.size(IconButtonDefaults.largeContainerSize()),
        shape = IconButtonDefaults.largeRoundShape,
    ) {
        Icon(
            Icons.Filled.Lock,
            contentDescription = description,
            modifier = Modifier.size(IconButtonDefaults.largeIconSize),
        )
    }
}

@Composable
fun TintedIconButtonSample() {
    val description = "Localized description"
    IconButton(onClick = { /* doSomething() */ }) {
        Icon(
            rememberVectorPainter(image = Icons.Filled.Lock),
            contentDescription = description,
            tint = Color.Red,
        )
    }
}

@Composable
fun IconToggleButtonSample() {
    var checked by remember { mutableStateOf(false) }
    val description = "Localized description"
    IconToggleButton(checked = checked, onCheckedChange = { checked = it }) {
        if (checked) {
            Icon(Icons.Filled.Lock, contentDescription = description)
        } else {
            Icon(Icons.Outlined.Lock, contentDescription = description)
        }
    }
}

@Composable
fun IconToggleButtonWithAnimatedShapeSample() {
    var checked by remember { mutableStateOf(false) }
    val description = "Localized description"
    IconToggleButton(
        checked = checked,
        onCheckedChange = { checked = it },
        shapes = IconButtonDefaults.toggleableShapes(),
    ) {
        if (checked) {
            Icon(Icons.Filled.Lock, contentDescription = description)
        } else {
            Icon(Icons.Outlined.Lock, contentDescription = description)
        }
    }
}

@Composable
fun FilledIconButtonSample() {
    val description = "Localized description"
    FilledIconButton(onClick = { /* doSomething() */ }) {
        Icon(Icons.Filled.Lock, contentDescription = description)
    }
}

@Composable
fun FilledIconButtonWithAnimatedShapeSample() {
    val description = "Localized description"
    FilledIconButton(onClick = { /* doSomething() */ }, shapes = IconButtonDefaults.shapes()) {
        Icon(Icons.Filled.Lock, contentDescription = description)
    }
}

@Composable
fun FilledIconToggleButtonSample() {
    var checked by remember { mutableStateOf(false) }
    val description = "Localized description"
    FilledIconToggleButton(checked = checked, onCheckedChange = { checked = it }) {
        if (checked) {
            Icon(Icons.Filled.Lock, contentDescription = description)
        } else {
            Icon(Icons.Outlined.Lock, contentDescription = description)
        }
    }
}

@Composable
fun FilledIconToggleButtonWithAnimatedShapeSample() {
    var checked by remember { mutableStateOf(false) }
    val description = "Localized description"
    FilledIconToggleButton(
        checked = checked,
        onCheckedChange = { checked = it },
        shapes = IconButtonDefaults.toggleableShapes(),
    ) {
        if (checked) {
            Icon(Icons.Filled.Lock, contentDescription = description)
        } else {
            Icon(Icons.Outlined.Lock, contentDescription = description)
        }
    }
}

@Composable
fun FilledTonalIconButtonSample() {
    val description = "Localized description"
    FilledTonalIconButton(onClick = { /* doSomething() */ }) {
        Icon(Icons.Filled.Lock, contentDescription = description)
    }
}

@Composable
fun FilledTonalIconButtonWithAnimatedShapeSample() {
    val description = "Localized description"
    FilledTonalIconButton(
        onClick = { /* doSomething() */ },
        shapes = IconButtonDefaults.shapes(),
    ) {
        Icon(Icons.Filled.Lock, contentDescription = description)
    }
}

@Composable
fun FilledTonalIconToggleButtonSample() {
    var checked by remember { mutableStateOf(false) }
    val description = "Localized description"
    FilledTonalIconToggleButton(checked = checked, onCheckedChange = { checked = it }) {
        if (checked) {
            Icon(Icons.Filled.Lock, contentDescription = description)
        } else {
            Icon(Icons.Outlined.Lock, contentDescription = description)
        }
    }
}

@Composable
fun FilledTonalIconToggleButtonWithAnimatedShapeSample() {
    var checked by remember { mutableStateOf(false) }
    val description = "Localized description"
    FilledTonalIconToggleButton(
        checked = checked,
        onCheckedChange = { checked = it },
        shapes = IconButtonDefaults.toggleableShapes(),
    ) {
        if (checked) {
            Icon(Icons.Filled.Lock, contentDescription = description)
        } else {
            Icon(Icons.Outlined.Lock, contentDescription = description)
        }
    }
}

@Composable
fun OutlinedIconButtonSample() {
    val description = "Localized description"
    OutlinedIconButton(onClick = { /* doSomething() */ }) {
        Icon(Icons.Filled.Lock, contentDescription = description)
    }
}

@Composable
fun OutlinedIconButtonWithAnimatedShapeSample() {
    val description = "Localized description"
    OutlinedIconButton(
        onClick = { /* doSomething() */ },
        shapes = IconButtonDefaults.shapes(),
    ) {
        Icon(Icons.Filled.Lock, contentDescription = description)
    }
}

@Composable
fun OutlinedIconToggleButtonSample() {
    var checked by remember { mutableStateOf(false) }
    val description = "Localized description"
    OutlinedIconToggleButton(checked = checked, onCheckedChange = { checked = it }) {
        if (checked) {
            Icon(Icons.Filled.Lock, contentDescription = description)
        } else {
            Icon(Icons.Outlined.Lock, contentDescription = description)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OutlinedIconToggleButtonWithAnimatedShapeSample() {
    var checked by remember { mutableStateOf(false) }
    val description = "Localized description"
    OutlinedIconToggleButton(
        checked = checked,
        onCheckedChange = { checked = it },
        shapes = IconButtonDefaults.toggleableShapes(),
    ) {
        if (checked) {
            Icon(Icons.Filled.Lock, contentDescription = description)
        } else {
            Icon(Icons.Outlined.Lock, contentDescription = description)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FloatingActionButtonSample() {
    FloatingActionButton(onClick = { /* do something */ }) {
        Icon(Icons.Filled.Add, "Localized description")
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SmallFloatingActionButtonSample() {
    SmallFloatingActionButton(onClick = { /* do something */ }) {
        Icon(Icons.Filled.Add, contentDescription = "Localized description")
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class, ExperimentalMaterial3Api::class)
@Composable
fun MediumFloatingActionButtonSample() {
    MediumFloatingActionButton(onClick = { /* do something */ }) {
        Icon(
            Icons.Filled.Add,
            contentDescription = "Localized description",
            modifier = Modifier.size(FloatingActionButtonDefaults.MediumIconSize),
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LargeFloatingActionButtonSample() {
    LargeFloatingActionButton(onClick = { /* do something */ }) {
        Icon(
            Icons.Filled.Add,
            contentDescription = "Localized description",
            modifier = Modifier.size(FloatingActionButtonDefaults.LargeIconSize),
        )
    }
}
