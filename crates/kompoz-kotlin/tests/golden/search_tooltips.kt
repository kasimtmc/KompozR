// Search bars and rich tooltips as Compose writes them (WP7): an object's member imported by name
// (`MaterialTheme.motionScheme` read as `motionScheme`); a file function's `(String) -> Unit` parameter given a
// lambda (its parameter a `String`, lent to a `&str`); a lambda of typed parameters kept in a `val` and called (a
// literal made the `String` it takes); `@Suppress("DEPRECATION")` on a function; a nullable parameter given an `if`
// of a `null` branch; a kept closure parameter passed on from a lambda that captured it; the search bars' `content`
// a ColumnScope and AppBarWithSearch's `actions` a RowScope (`AnimatedVisibility` in them the scope's); a
// `const val` of literals joined by `+`; RichTooltip in a tooltip box's TooltipScope.

import androidx.compose.material3.MaterialTheme.motionScheme

const val tipText =
    "Configure permissions. " +
        "Visit go/permissions for details"

@Composable
private fun Results(onResultClick: (String) -> Unit, modifier: Modifier = Modifier) {
    Column(modifier) { Text("Suggestion", Modifier.clickable { onResultClick("Suggestion") }) }
}

@Suppress("DEPRECATION")
@Composable
fun Searching(pinned: Boolean, onMenu: () -> Unit) {
    val searchBarState = rememberSearchBarState()
    val textFieldState = rememberTextFieldState()
    val scrollBehavior = SearchBarDefaults.enterAlwaysSearchBarScrollBehavior()
    val field = @Composable { tag: String ->
        SearchBarDefaults.InputField(
            textFieldState = textFieldState,
            searchBarState = searchBarState,
            onSearch = {},
            modifier = Modifier.testTag(tag),
        )
    }
    Column(Modifier.nestedScroll(scrollBehavior.nestedScrollConnection)) {
        AppBarWithSearch(
            state = searchBarState,
            inputField = { field("bar") },
            navigationIcon = { IconButton(onClick = onMenu) { Icon(Icons.Filled.Menu, contentDescription = "Menu") } },
            actions = {
                AnimatedVisibility(visible = searchBarState.currentValue == SearchBarValue.Collapsed) {
                    Icon(Icons.Filled.Person, contentDescription = "Account")
                }
            },
            scrollBehavior = if (pinned) null else scrollBehavior,
        )
        TopSearchBar(state = searchBarState, inputField = { field("top") })
        AnimatedVisibility(
            visible = searchBarState.targetValue == SearchBarValue.Collapsed,
            enter = slideIn(animationSpec = motionScheme.fastSpatialSpec(), initialOffset = { IntOffset(-it.width, 0) }),
        ) {
            Text(tipText)
        }
    }
    ExpandedFullScreenSearchBar(state = searchBarState, inputField = { field("view") }) {
        AnimatedVisibility(visible = true) { Text("Results") }
        Results(onResultClick = { result -> textFieldState.setTextAndPlaceCursorAtEnd(result) })
    }
}

@Composable
fun RichTip() {
    val state = rememberTooltipState(isPersistent = true)
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = {
            RichTooltip(
                title = { Text("Title") },
                action = { TextButton(onClick = { state.dismiss() }) { Text("OK") } },
                caretShape = TooltipDefaults.caretShape(),
                colors = TooltipDefaults.richTooltipColors(containerColor = Color.Red),
            ) {
                Text(tipText)
            }
        },
        state = state,
        hasAction = true,
    ) {
        Icon(Icons.Filled.Info, contentDescription = "Info")
    }
}
