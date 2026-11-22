// Material's selection controls as Compose's samples write them (material3 1.5's CheckboxSamples,
// RadioButtonSamples, SwitchSamples), and their colors given through XDefaults.colors(…).

@Composable
fun CheckboxSample() {
    val checkedState = remember { mutableStateOf(true) }
    Checkbox(checked = checkedState.value, onCheckedChange = { checkedState.value = it })
}

@Composable
fun CheckboxWithTextSample() {
    val (checkedState, onStateChange) = remember { mutableStateOf(true) }
    val interactionSource = remember { MutableInteractionSource() }
    Row(
        Modifier.fillMaxWidth()
            .height(56.dp)
            .toggleable(
                value = checkedState,
                onValueChange = { onStateChange(!checkedState) },
                role = Role.Checkbox,
                indication = null,
                interactionSource = interactionSource,
            )
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(
            checked = checkedState,
            interactionSource = interactionSource,
            onCheckedChange = onStateChange,
        )
        Text(
            text = "Option selection",
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier.padding(start = 16.dp),
        )
    }
}

@Composable
fun CheckboxRoundedStrokesSample() {
    val strokeWidthPx = with(LocalDensity.current) { floor(CheckboxDefaults.StrokeWidth.toPx()) }
    val checkmarkStroke =
        remember(strokeWidthPx) {
            Stroke(width = strokeWidthPx, cap = StrokeCap.Round, join = StrokeJoin.Round)
        }
    val outlineStroke = remember(strokeWidthPx) { Stroke(width = strokeWidthPx) }
    val checkedState = remember { mutableStateOf(true) }
    Checkbox(
        checked = checkedState.value,
        onCheckedChange = { checkedState.value = it },
        checkmarkStroke = checkmarkStroke,
        outlineStroke = outlineStroke,
    )
}

@Composable
fun TriStateCheckboxSample() {
    Column {
        // define dependent checkboxes states
        val (state, onStateChange) = remember { mutableStateOf(true) }
        val (state2, onStateChange2) = remember { mutableStateOf(true) }

        // TriStateCheckbox state reflects state of dependent checkboxes
        val parentState =
            remember(state, state2) {
                if (state && state2) ToggleableState.On
                else if (!state && !state2) ToggleableState.Off else ToggleableState.Indeterminate
            }
        // click on TriStateCheckbox can set state for dependent checkboxes
        val onParentClick = {
            val s = parentState != ToggleableState.On
            onStateChange(s)
            onStateChange2(s)
        }

        // The sample below composes just basic checkboxes which are not fully accessible on their
        // own. See the CheckboxWithTextSample as a way to ensure your checkboxes are fully
        // accessible.
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier =
                Modifier.triStateToggleable(
                    state = parentState,
                    onClick = onParentClick,
                    role = Role.Checkbox,
                ),
        ) {
            TriStateCheckbox(state = parentState, onClick = null)
            Text("Receive Emails")
        }
        Spacer(Modifier.size(25.dp))
        Column(Modifier.padding(24.dp, 0.dp, 0.dp, 0.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier =
                    Modifier.toggleable(
                        value = state,
                        onValueChange = onStateChange,
                        role = Role.Checkbox,
                    ),
            ) {
                Checkbox(state, null)
                Text("Daily")
            }
            Spacer(Modifier.size(25.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier =
                    Modifier.toggleable(
                        value = state2,
                        onValueChange = onStateChange2,
                        role = Role.Checkbox,
                    ),
            ) {
                Checkbox(state2, null)
                Text("Weekly")
            }
        }
    }
}

@Composable
fun TriStateCheckboxRoundedStrokesSample() {
    val strokeWidthPx = with(LocalDensity.current) { floor(CheckboxDefaults.StrokeWidth.toPx()) }
    val checkmarkStroke =
        remember(strokeWidthPx) {
            Stroke(width = strokeWidthPx, cap = StrokeCap.Round, join = StrokeJoin.Round)
        }
    val outlineStroke = remember(strokeWidthPx) { Stroke(width = strokeWidthPx) }
    Column {
        // define dependent checkboxes states
        val (state, onStateChange) = remember { mutableStateOf(true) }
        val (state2, onStateChange2) = remember { mutableStateOf(true) }

        // TriStateCheckbox state reflects state of dependent checkboxes
        val parentState =
            remember(state, state2) {
                if (state && state2) ToggleableState.On
                else if (!state && !state2) ToggleableState.Off else ToggleableState.Indeterminate
            }
        // click on TriStateCheckbox can set state for dependent checkboxes
        val onParentClick = {
            val s = parentState != ToggleableState.On
            onStateChange(s)
            onStateChange2(s)
        }

        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier =
                Modifier.triStateToggleable(
                    state = parentState,
                    onClick = onParentClick,
                    role = Role.Checkbox,
                ),
        ) {
            TriStateCheckbox(
                state = parentState,
                onClick = null,
                checkmarkStroke = checkmarkStroke,
                outlineStroke = outlineStroke,
            )
            Text("Receive Emails")
        }
        Spacer(Modifier.size(25.dp))
        Column(Modifier.padding(24.dp, 0.dp, 0.dp, 0.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier =
                    Modifier.toggleable(
                        value = state,
                        onValueChange = onStateChange,
                        role = Role.Checkbox,
                    ),
            ) {
                Checkbox(
                    checked = state,
                    onCheckedChange = null,
                    checkmarkStroke = checkmarkStroke,
                    outlineStroke = outlineStroke,
                )
                Text("Daily")
            }
            Spacer(Modifier.size(25.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier =
                    Modifier.toggleable(
                        value = state2,
                        onValueChange = onStateChange2,
                        role = Role.Checkbox,
                    ),
            ) {
                Checkbox(
                    checked = state2,
                    onCheckedChange = null,
                    checkmarkStroke = checkmarkStroke,
                    outlineStroke = outlineStroke,
                )
                Text("Weekly")
            }
        }
    }
}

@Composable
fun RadioButtonSample() {
    // We have two radio buttons and only one can be selected
    var state by remember { mutableStateOf(true) }
    // Note that Modifier.selectableGroup() is essential to ensure correct accessibility behavior.
    // We also set a content description for this sample, but note that a RadioButton would usually
    // be part of a higher level component, such as a raw with text, and that component would need
    // to provide an appropriate content description. See RadioGroupSample.
    Row(Modifier.selectableGroup()) {
        RadioButton(
            selected = state,
            onClick = { state = true },
            modifier = Modifier.semantics { contentDescription = "Localized Description" },
        )
        RadioButton(
            selected = !state,
            onClick = { state = false },
            modifier = Modifier.semantics { contentDescription = "Localized Description" },
        )
    }
}

@Composable
fun RadioGroupSample() {
    val radioOptions = listOf("Calls", "Missed", "Friends")
    val (selectedOption, onOptionSelected) = remember { mutableStateOf(radioOptions[0]) }
    // Note that Modifier.selectableGroup() is essential to ensure correct accessibility behavior
    Column(Modifier.selectableGroup()) {
        radioOptions.forEach { text ->
            Row(
                Modifier.fillMaxWidth()
                    .height(56.dp)
                    .selectable(
                        selected = (text == selectedOption),
                        onClick = { onOptionSelected(text) },
                        role = Role.RadioButton,
                    )
                    .padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                RadioButton(
                    selected = (text == selectedOption),
                    onClick = null, // null recommended for accessibility with screenreaders
                )
                Text(
                    text = text,
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.padding(start = 16.dp),
                )
            }
        }
    }
}

@Composable
fun SwitchSample() {
    var checked by remember { mutableStateOf(true) }
    Switch(
        modifier = Modifier.semantics { contentDescription = "Demo" },
        checked = checked,
        onCheckedChange = { checked = it },
    )
}

@Composable
fun SwitchWithThumbIconSample() {
    var checked by remember { mutableStateOf(true) }

    Switch(
        modifier = Modifier.semantics { contentDescription = "Demo with icon" },
        checked = checked,
        onCheckedChange = { checked = it },
        thumbContent = {
            if (checked) {
                // Icon isn't focusable, no need for content description
                Icon(
                    imageVector = Icons.Filled.Check,
                    contentDescription = null,
                    modifier = Modifier.size(SwitchDefaults.IconSize),
                )
            }
        },
    )
}

// Colors given in their places, as an app writes them.
@Composable
fun ColoredControls(enabled: Boolean) {
    var checked by remember { mutableStateOf(true) }
    var selected by remember { mutableStateOf(true) }
    Column {
        Checkbox(
            checked = checked,
            onCheckedChange = { checked = it },
            modifier = Modifier.testTag("kutu"),
            enabled = enabled,
            colors = CheckboxDefaults.colors(checkedColor = Color.Blue, checkmarkColor = Color.Yellow),
        )
        TriStateCheckbox(
            state = ToggleableState.Indeterminate,
            onClick = {},
            modifier = Modifier.testTag("üç"),
            colors = CheckboxDefaults.colors(checkedBoxColor = Color.Green, checkedBorderColor = Color.Green),
        )
        RadioButton(
            selected = selected,
            onClick = { selected = !selected },
            modifier = Modifier.testTag("radyo"),
            enabled = enabled,
            colors = RadioButtonDefaults.colors(selectedColor = Color.Magenta, disabledSelectedColor = Color.Gray),
        )
        Switch(
            checked = checked,
            onCheckedChange = { checked = it },
            modifier = Modifier.testTag("anahtar"),
            enabled = enabled,
            colors = SwitchDefaults.colors(checkedTrackColor = Color.Red, checkedThumbColor = Color.White),
        )
        Text(if (CheckboxDefaults.colors() == CheckboxDefaults.colors().copy()) "aynı" else "farklı")
    }
}
