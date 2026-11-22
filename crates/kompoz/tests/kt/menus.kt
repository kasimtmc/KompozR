// Material's menus in a Kotlin file of their own (kompoz_file!): material3 1.5's MenuSamples.kt and
// ExposedDropdownMenuSamples.kt as Compose writes them — test tags added to find them. GroupedMenuSample's
// DropdownMenuButtonGroup (a ButtonGroup, WP1b's) as written. ExposedDropdownMenuSamples' private helpers (`filteredBy` and the multi-autocomplete
// sample's `TextFieldState` extensions) are local functions there, whose `return null` leaves an inline lambda:
// Kotlin's local functions and non-local returns come with stage 14, so they are the file's own private functions
// here, the lambda's `return` an `if` (the samples' composables are as Compose writes them).
@file:OptIn(ExperimentalMaterial3Api::class)

package com.example.menus

import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.input.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.*
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.util.fastForEachIndexed

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun MenuSample() {
    var expanded by remember { mutableStateOf(false) }

    Box(modifier = Modifier.fillMaxSize().wrapContentSize(Alignment.TopStart)) {
        TooltipBox(
            positionProvider =
                TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = { PlainTooltip { Text("Localized description") } },
            state = rememberTooltipState(),
        ) {
            IconButton(onClick = { expanded = true }) {
                Icon(Icons.Default.MoreVert, contentDescription = "Localized description")
            }
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text("Edit") },
                onClick = { /* Handle edit! */ },
                leadingIcon = { Icon(Icons.Outlined.Edit, contentDescription = null) },
            )
            DropdownMenuItem(
                text = { Text("Settings") },
                onClick = { /* Handle settings! */ },
                leadingIcon = { Icon(Icons.Outlined.Settings, contentDescription = null) },
            )
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text("Send Feedback") },
                onClick = { /* Handle send feedback! */ },
                leadingIcon = { Icon(Icons.Outlined.Email, contentDescription = null) },
                trailingIcon = { Text("F11", textAlign = TextAlign.Center) },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun GroupedMenuSample() {
    val groupInteractionSource = remember { MutableInteractionSource() }
    var expanded by remember { mutableStateOf(false) }
    var homeChecked by remember { mutableStateOf(false) }
    val groupLabels = listOf("Modification", "Navigation")
    val groupItemLabels = listOf(listOf("Edit", "Settings"), listOf("Home", "More Options"))
    val groupItemLeadingIcons =
        listOf(
            listOf(Icons.Outlined.Edit, Icons.Outlined.Settings),
            listOf(null, Icons.Outlined.Info),
        )
    val groupItemCheckedLeadingIcons =
        listOf(
            listOf(Icons.Filled.Edit, Icons.Filled.Settings),
            listOf(Icons.Filled.Check, Icons.Filled.Info),
        )
    val groupItemTrailingIcons: List<List<ImageVector?>> =
        listOf(listOf(null, null), listOf(Icons.Outlined.Home, Icons.Outlined.MoreVert))
    val groupItemCheckedTrailingIcons: List<List<ImageVector?>> =
        listOf(listOf(null, null), listOf(Icons.Filled.Home, Icons.Filled.MoreVert))
    val groupItemSupportingText: List<List<String?>> =
        listOf(listOf("Edit mode", null), listOf(null, "Opens menu"))
    val checked = remember {
        listOf(mutableStateListOf(false, false), mutableStateListOf(false, false))
    }

    Box(modifier = Modifier.fillMaxSize().wrapContentSize(Alignment.TopStart)) {
        TooltipBox(
            positionProvider =
                TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = { PlainTooltip { Text("Localized description") } },
            state = rememberTooltipState(),
        ) {
            IconButton(onClick = { expanded = true }) {
                Icon(Icons.Default.MoreVert, contentDescription = "Localized description")
            }
        }
        DropdownMenuPopup(expanded = expanded, onDismissRequest = { expanded = false }) {
            val groupCount = groupLabels.size
            groupLabels.fastForEachIndexed { groupIndex, label ->
                DropdownMenuGroup(
                    shapes = MenuDefaults.groupShape(groupIndex, groupCount),
                    interactionSource = groupInteractionSource,
                ) {
                    MenuDefaults.Label { Text(label) }
                    HorizontalDivider(
                        modifier = Modifier.padding(MenuDefaults.HorizontalDividerPadding)
                    )
                    val groupItemCount = groupItemLabels[groupIndex].size
                    groupItemLabels[groupIndex].fastForEachIndexed { itemIndex, itemLabel ->
                        DropdownMenuItem(
                            text = { Text(itemLabel) },
                            supportingText =
                                groupItemSupportingText[groupIndex][itemIndex]?.let { supportingText
                                    ->
                                    { Text(supportingText) }
                                },
                            shapes = MenuDefaults.itemShape(itemIndex, groupItemCount),
                            leadingIcon =
                                groupItemLeadingIcons[groupIndex][itemIndex]?.let { iconData ->
                                    {
                                        Icon(
                                            iconData,
                                            modifier = Modifier.size(MenuDefaults.LeadingIconSize),
                                            contentDescription = null,
                                        )
                                    }
                                },
                            checkedLeadingIcon = {
                                Icon(
                                    groupItemCheckedLeadingIcons[groupIndex][itemIndex],
                                    modifier = Modifier.size(MenuDefaults.LeadingIconSize),
                                    contentDescription = null,
                                )
                            },
                            trailingIcon =
                                if (checked[groupIndex][itemIndex]) {
                                    groupItemCheckedTrailingIcons[groupIndex][itemIndex]?.let {
                                        iconData ->
                                        {
                                            Icon(
                                                iconData,
                                                modifier =
                                                    Modifier.size(MenuDefaults.TrailingIconSize),
                                                contentDescription = null,
                                            )
                                        }
                                    }
                                } else {
                                    groupItemTrailingIcons[groupIndex][itemIndex]?.let { iconData ->
                                        {
                                            Icon(
                                                iconData,
                                                modifier =
                                                    Modifier.size(MenuDefaults.TrailingIconSize),
                                                contentDescription = null,
                                            )
                                        }
                                    }
                                },
                            checked = checked[groupIndex][itemIndex],
                            onCheckedChange = { checked[groupIndex][itemIndex] = it },
                        )
                    }
                }

                if (groupIndex != groupCount - 1) {
                    Spacer(Modifier.height(MenuDefaults.GroupSpacing))
                }
            }
            if (checked.last().last()) {
                DropdownMenuButtonGroup()
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun DropdownMenuButtonGroup() {
    ButtonGroup(
        overflowIndicator = { menuState -> ButtonGroupDefaults.OverflowIndicator(menuState) },
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(2.dp, Alignment.CenterHorizontally),
    ) {
        customItem(
            buttonGroupContent = {
                FilledIconButton(
                    onClick = {},
                    colors =
                        IconButtonDefaults.filledIconButtonColors(
                            containerColor = MenuDefaults.groupStandardContainerColor
                        ),
                ) {
                    Icon(Icons.Filled.ThumbUp, contentDescription = "Localized description")
                }
            },
            menuContent = {
                DropdownMenuItem(
                    leadingIcon = {
                        Icon(Icons.Filled.ThumbUp, contentDescription = "Localized description")
                    },
                    text = { Text("Thumbs up") },
                    onClick = {},
                )
            },
        )

        customItem(
            buttonGroupContent = {
                FilledIconButton(
                    onClick = {},
                    colors =
                        IconButtonDefaults.filledIconButtonColors(
                            containerColor = MenuDefaults.groupStandardContainerColor
                        ),
                ) {
                    Icon(Icons.Filled.ThumbDown, contentDescription = "Localized description")
                }
            },
            menuContent = {
                DropdownMenuItem(
                    leadingIcon = {
                        Icon(Icons.Filled.ThumbDown, contentDescription = "Localized description")
                    },
                    text = { Text("Thumbs down") },
                    onClick = {},
                )
            },
        )

        customItem(
            buttonGroupContent = {
                FilledIconButton(
                    onClick = {},
                    colors =
                        IconButtonDefaults.filledIconButtonColors(
                            containerColor = MenuDefaults.groupStandardContainerColor
                        ),
                ) {
                    Icon(Icons.Filled.TagFaces, contentDescription = "Localized description")
                }
            },
            menuContent = {
                DropdownMenuItem(
                    leadingIcon = {
                        Icon(Icons.Filled.TagFaces, contentDescription = "Localized description")
                    },
                    text = { Text("Emotes") },
                    onClick = {},
                )
            },
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MenuWithScrollStateSample() {
    var expanded by remember { mutableStateOf(false) }
    val scrollState = rememberScrollState()
    Box(modifier = Modifier.fillMaxSize().wrapContentSize(Alignment.TopStart)) {
        TooltipBox(
            positionProvider =
                TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = { PlainTooltip { Text("Localized description") } },
            state = rememberTooltipState(),
        ) {
            IconButton(onClick = { expanded = true }) {
                Icon(Icons.Default.MoreVert, contentDescription = "Localized description")
            }
        }
        DropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false },
            scrollState = scrollState,
        ) {
            repeat(30) {
                DropdownMenuItem(
                    text = { Text("Item ${it + 1}") },
                    onClick = { /* TODO */ },
                    leadingIcon = { Icon(Icons.Outlined.Edit, contentDescription = null) },
                )
            }
        }
        LaunchedEffect(expanded) {
            if (expanded) {
                // Scroll to show the bottom menu items.
                scrollState.scrollTo(scrollState.maxValue)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ExposedDropdownMenuSample() {
    val options: List<String> = SampleData.take(5)
    var expanded by remember { mutableStateOf(false) }
    val textFieldState = rememberTextFieldState(options[0])
    var checkedIndex: Int? by remember { mutableStateOf(null) }

    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }) {
        TextField(
            // The `menuAnchor` modifier must be passed to the text field to handle
            // expanding/collapsing the menu on click. A read-only text field has
            // the anchor type `PrimaryNotEditable`.
            modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
            state = textFieldState,
            readOnly = true,
            lineLimits = TextFieldLineLimits.SingleLine,
            label = { Text("Label") },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            colors = ExposedDropdownMenuDefaults.textFieldColors(),
        )
        ExposedDropdownMenu(
            expanded = expanded,
            onDismissRequest = { expanded = false },
            containerColor = MenuDefaults.groupStandardContainerColor,
            shape = MenuDefaults.standaloneGroupShape,
        ) {
            val optionCount = options.size
            options.forEachIndexed { index, option ->
                DropdownMenuItem(
                    shapes = MenuDefaults.itemShape(index, optionCount),
                    text = { Text(option, style = MaterialTheme.typography.bodyLarge) },
                    selected = index == checkedIndex,
                    onClick = {
                        textFieldState.setTextAndPlaceCursorAtEnd(option)
                        checkedIndex = index
                    },
                    selectedLeadingIcon = {
                        Icon(
                            Icons.Filled.Check,
                            modifier = Modifier.size(MenuDefaults.LeadingIconSize),
                            contentDescription = null,
                        )
                    },
                    contentPadding = ExposedDropdownMenuDefaults.ItemContentPadding,
                )
            }
        }
    }
}

@Composable
fun EditableExposedDropdownMenuSample() {
    val options: List<String> = SampleData
    val textFieldState = rememberTextFieldState()

    // The text that the user inputs into the text field can be used to filter the options.
    // This sample uses string subsequence matching.
    val filteredOptions = options.filteredBy(textFieldState.text)

    val (allowExpanded, setExpanded) = remember { mutableStateOf(false) }
    val expanded = allowExpanded && filteredOptions.isNotEmpty()

    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = setExpanded) {
        TextField(
            // The `menuAnchor` modifier must be passed to the text field to handle
            // expanding/collapsing the menu on click. An editable text field has
            // the anchor type `PrimaryEditable`.
            modifier =
                Modifier.width(280.dp).menuAnchor(ExposedDropdownMenuAnchorType.PrimaryEditable),
            state = textFieldState,
            lineLimits = TextFieldLineLimits.SingleLine,
            label = { Text("Label") },
            trailingIcon = {
                ExposedDropdownMenuDefaults.TrailingIcon(
                    expanded = expanded,
                    // If the text field is editable, it is recommended to make the
                    // trailing icon a `menuAnchor` of type `SecondaryEditable`. This
                    // provides a better experience for certain accessibility services
                    // to choose a menu option without typing.
                    modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.SecondaryEditable),
                )
            },
            colors = ExposedDropdownMenuDefaults.textFieldColors(),
        )
        ExposedDropdownMenu(
            modifier = Modifier.heightIn(max = 280.dp),
            expanded = expanded,
            onDismissRequest = { setExpanded(false) },
        ) {
            filteredOptions.forEach { option ->
                DropdownMenuItem(
                    text = { Text(option, style = MaterialTheme.typography.bodyLarge) },
                    onClick = {
                        textFieldState.setTextAndPlaceCursorAtEnd(option.text)
                        setExpanded(false)
                    },
                    contentPadding = ExposedDropdownMenuDefaults.ItemContentPadding,
                )
            }
        }
    }
}

/**
 * Returns the TextRange of the current token around the cursor, where commas define token
 * boundaries.
 */
private fun TextFieldState.currentTokenRange(): TextRange? {
    if (!selection.collapsed) return null

    val cursor = selection.start
    var start = cursor
    while (start > 0 && text[start - 1] != ',') {
        start--
    }
    while (start < cursor && text[start] == ' ') {
        start++
    }

    var end = cursor
    while (end < text.length && text[end] != ',') {
        end++
    }
    return TextRange(start, end)
}

private fun TextFieldState.replaceThenAddComma(start: Int, end: Int, text: CharSequence) = edit {
    replace(start, end, text)
    val afterText = start + text.length
    if (afterText == this.length || this.charAt(afterText) != ',') {
        insert(afterText, ", ")
        placeCursorBeforeCharAt(afterText + 2)
    } else {
        placeCursorAfterCharAt(afterText)
    }
}

@Composable
fun MultiAutocompleteExposedDropdownMenuSample() {
    val allOptions: List<String> = SampleData
    val textFieldState = rememberTextFieldState()
    val tokenSelection = textFieldState.currentTokenRange()
    val tokenAtCursor =
        if (tokenSelection != null) textFieldState.text.substring(tokenSelection) else ""
    val filteredOptions =
        if (tokenAtCursor.isBlank()) emptyList() else allOptions.filteredBy(tokenAtCursor)

    val (allowExpanded, setExpanded) = remember { mutableStateOf(false) }
    val expanded = allowExpanded && filteredOptions.isNotEmpty()

    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = setExpanded) {
        TextField(
            modifier =
                Modifier.width(280.dp).menuAnchor(ExposedDropdownMenuAnchorType.PrimaryEditable),
            state = textFieldState,
            lineLimits = TextFieldLineLimits.SingleLine,
            label = { Text("Label") },
            trailingIcon = {
                ExposedDropdownMenuDefaults.TrailingIcon(
                    expanded = expanded,
                    modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.SecondaryEditable),
                )
            },
            colors = ExposedDropdownMenuDefaults.textFieldColors(),
        )
        ExposedDropdownMenu(
            modifier = Modifier.heightIn(max = 280.dp),
            expanded = expanded,
            onDismissRequest = { setExpanded(false) },
        ) {
            filteredOptions.forEach { option ->
                DropdownMenuItem(
                    text = { Text(option, style = MaterialTheme.typography.bodyLarge) },
                    onClick = {
                        if (tokenSelection != null) {
                            textFieldState.replaceThenAddComma(
                                tokenSelection.start,
                                tokenSelection.end,
                                option,
                            )
                        }
                    },
                    contentPadding = ExposedDropdownMenuDefaults.ItemContentPadding,
                )
            }
        }
    }
}

/**
 * Returns the elements of [this] list that contain [text] as a subsequence, with the subsequence
 * underlined as an [AnnotatedString].
 */
private fun List<String>.filteredBy(text: CharSequence): List<AnnotatedString> {
    return this.mapNotNull { option -> underlineSubsequence(text, option) }
}

private fun underlineSubsequence(needle: CharSequence, haystack: String): AnnotatedString? {
    var found = true
    val underlined = buildAnnotatedString {
        var i = 0
        for (char in needle) {
            val start = i
            val at = haystack.indexOf(char, startIndex = i, ignoreCase = true)
            if (at < 0) {
                found = false
                break
            }
            i = at
            append(haystack.substring(start, i))
            withStyle(SpanStyle(textDecoration = TextDecoration.Underline)) {
                append(haystack[i])
            }
            i += 1
        }
        append(haystack.substring(i, haystack.length))
    }
    return if (found) underlined else null
}

private val SampleData =
    listOf(
        "Android",
        "Base",
        "Cupcake",
        "Donut",
        "Eclair",
        "Froyo",
        "Gingerbread",
        "Honeycomb",
        "Ice Cream Sandwich",
        "Jelly Bean",
        "KitKat",
        "Lollipop",
        "Marshmallow",
        "Nougat",
        "Oreo",
        "Pie",
        "Quince Tart",
        "Red Velvet Cake",
        "Snow Cone",
        "Tiramisu",
        "Upside Down Cake",
        "Vanilla Ice Cream",
        "W123",
        "X456",
        "Y789",
        "Z000",
    )

// ExposedDropdownMenuBox's documentation sample as Compose 1.5 writes it: options given to `forEach`'s `it`, the
// anchor a filling read-only TextField whose value is a String.
@Composable
fun ExposedDropdownMenuOfStrings(options: List<String>) {
    var expanded by remember { mutableStateOf(false) }
    var text by remember { mutableStateOf(options[0]) }
    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }, modifier = Modifier.width(300.dp)) {
        TextField(
            modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable).fillMaxWidth().testTag("dizgi"),
            value = text,
            onValueChange = {},
            readOnly = true,
            singleLine = true,
            label = { Text("Seçenek") },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            colors = ExposedDropdownMenuDefaults.textFieldColors(),
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }, scrollState = rememberScrollState()) {
            options.forEach {
                DropdownMenuItem(
                    text = { Text(it) },
                    onClick = {
                        text = it
                        expanded = false
                    },
                    contentPadding = ExposedDropdownMenuDefaults.ItemContentPadding,
                )
            }
        }
    }
}

// An anchor of the deprecated `MenuAnchorType` (Compose 1.4's name of ExposedDropdownMenuAnchorType), a menu in
// MenuDefaults' shape and item colors, a shaped item and a checkable one.
@Suppress("DEPRECATION")
@Composable
fun OldAnchorMenu() {
    var expanded by remember { mutableStateOf(false) }
    var checked by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }) {
        TextField(
            state = rememberTextFieldState("eski"),
            readOnly = true,
            modifier = Modifier.menuAnchor(MenuAnchorType.PrimaryNotEditable),
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }, shape = MenuDefaults.shape) {
            DropdownMenuItem(
                onClick = { expanded = false },
                text = { Text("Biçimli") },
                shape = MenuDefaults.standaloneItemShape,
                colors = MenuDefaults.itemColors(textColor = Color.Red),
            )
            DropdownMenuItem(
                checked = checked,
                onCheckedChange = { checked = it },
                text = { Text("İşaretli") },
                shapes = MenuDefaults.itemShape(1, 2),
            )
        }
    }
}
