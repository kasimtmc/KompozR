// Material's search bars as Compose's samples write them: material3 1.5's SearchBarSamples.kt as written (its
// @Sampled/@Preview/@OptIn annotations left out; its imports kept: `motionScheme` is MaterialTheme's, imported), and
// each public search API by Kotlin's named arguments.

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.slideIn
import androidx.compose.animation.slideOut
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.AccountCircle
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.AppBarWithSearch
import androidx.compose.material3.ExpandedDockedSearchBarWithGap
import androidx.compose.material3.ExpandedFullScreenContainedSearchBar
import androidx.compose.material3.ExpandedFullScreenSearchBar
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme.motionScheme
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.SearchBar
import androidx.compose.material3.SearchBarDefaults
import androidx.compose.material3.SearchBarState
import androidx.compose.material3.SearchBarValue
import androidx.compose.material3.Text
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberContainedSearchBarState
import androidx.compose.material3.rememberSearchBarState
import androidx.compose.material3.rememberTooltipState
import androidx.compose.material3.rememberWithGapSearchBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

// ---- SearchBarSamples.kt ----

@Composable
fun SimpleSearchBarSample() {
    val searchBarState = rememberSearchBarState()
    val textFieldState = rememberTextFieldState()
    val scope = rememberCoroutineScope()
    val inputField =
        @Composable {
            SearchBarDefaults.InputField(
                textFieldState = textFieldState,
                searchBarState = searchBarState,
                onSearch = { scope.launch { searchBarState.animateToCollapsed() } },
                placeholder = {
                    Text(modifier = Modifier.clearAndSetSemantics {}, text = "Search")
                },
                leadingIcon = { SampleLeadingIcon(searchBarState, scope) },
                trailingIcon = { SampleTrailingIcon() },
            )
        }

    SearchBar(state = searchBarState, inputField = inputField)
    ExpandedFullScreenSearchBar(state = searchBarState, inputField = inputField) {
        SampleSearchResults(
            onResultClick = { result ->
                textFieldState.setTextAndPlaceCursorAtEnd(result)
                scope.launch { searchBarState.animateToCollapsed() }
            }
        )
    }
}

@Composable
fun FullScreenSearchBarScaffoldSample() {
    val textFieldState = rememberTextFieldState()
    val searchBarState = rememberContainedSearchBarState()
    val scope = rememberCoroutineScope()
    val scrollBehavior = SearchBarDefaults.enterAlwaysSearchBarScrollBehavior()
    val appBarWithSearchColors =
        SearchBarDefaults.appBarWithSearchColors(
            searchBarColors = SearchBarDefaults.containedColors(state = searchBarState)
        )
    val inputField =
        @Composable {
            SearchBarDefaults.InputField(
                textFieldState = textFieldState,
                searchBarState = searchBarState,
                colors = appBarWithSearchColors.searchBarColors.inputFieldColors,
                onSearch = { scope.launch { searchBarState.animateToCollapsed() } },
                placeholder = {
                    Text(modifier = Modifier.clearAndSetSemantics {}, text = "Search")
                },
                leadingIcon = { SampleLeadingIcon(searchBarState, scope) },
                trailingIcon = { SampleTrailingIcon() },
            )
        }

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            AppBarWithSearch(
                scrollBehavior = scrollBehavior,
                state = searchBarState,
                colors = appBarWithSearchColors,
                inputField = inputField,
                navigationIcon = { SampleNavigationIcon(searchBarState, isAnimated = true) },
                actions = { SampleActions(searchBarState, isAnimated = true) },
            )
            ExpandedFullScreenContainedSearchBar(
                state = searchBarState,
                inputField = inputField,
                colors = appBarWithSearchColors.searchBarColors,
            ) {
                SampleSearchResults(
                    onResultClick = { result ->
                        textFieldState.setTextAndPlaceCursorAtEnd(result)
                        scope.launch { searchBarState.animateToCollapsed() }
                    }
                )
            }
        },
    ) { padding ->
        LazyColumn(contentPadding = padding, verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val list = List(100) { "Text $it" }
            items(count = list.size) {
                Text(
                    text = list[it],
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                )
            }
        }
    }
}

@Composable
fun DockedSearchBarScaffoldSample() {
    val textFieldState = rememberTextFieldState()
    val searchBarState = rememberWithGapSearchBarState()
    val scope = rememberCoroutineScope()
    val scrollBehavior = SearchBarDefaults.enterAlwaysSearchBarScrollBehavior()
    val appBarWithSearchColors = SearchBarDefaults.appBarWithSearchColors()
    val inputField =
        @Composable {
            SearchBarDefaults.InputField(
                textFieldState = textFieldState,
                searchBarState = searchBarState,
                colors = appBarWithSearchColors.searchBarColors.inputFieldColors,
                onSearch = { scope.launch { searchBarState.animateToCollapsed() } },
                placeholder = {
                    Text(modifier = Modifier.clearAndSetSemantics {}, text = "Search")
                },
                leadingIcon = { SampleLeadingIcon(searchBarState, scope) },
                trailingIcon = { SampleTrailingIcon() },
            )
        }

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            AppBarWithSearch(
                scrollBehavior = scrollBehavior,
                state = searchBarState,
                colors = appBarWithSearchColors,
                inputField = inputField,
                navigationIcon = { SampleNavigationIcon(searchBarState) },
                actions = { SampleActions(searchBarState) },
            )
            ExpandedDockedSearchBarWithGap(state = searchBarState, inputField = inputField) {
                SampleSearchResults(
                    onResultClick = { result ->
                        textFieldState.setTextAndPlaceCursorAtEnd(result)
                        scope.launch { searchBarState.animateToCollapsed() }
                    }
                )
            }
        },
    ) { padding ->
        LazyColumn(contentPadding = padding, verticalArrangement = Arrangement.spacedBy(8.dp)) {
            val list = List(100) { "Text $it" }
            items(count = list.size) {
                Text(
                    text = list[it],
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                )
            }
        }
    }
}

@Composable
private fun SampleSearchResults(onResultClick: (String) -> Unit, modifier: Modifier = Modifier) {
    Column(modifier.verticalScroll(rememberScrollState())) {
        repeat(10) { idx ->
            val resultText = "Suggestion $idx"
            ListItem(
                headlineContent = { Text(resultText) },
                supportingContent = { Text("Additional info") },
                leadingContent = { Icon(Icons.Filled.Star, contentDescription = null) },
                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                modifier =
                    Modifier.clickable { onResultClick(resultText) }
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 4.dp),
            )
        }
    }
}

@Composable
private fun SampleLeadingIcon(searchBarState: SearchBarState, scope: CoroutineScope) =
    if (searchBarState.currentValue == SearchBarValue.Expanded) {
        TooltipBox(
            positionProvider =
                TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = { PlainTooltip { Text("Back") } },
            state = rememberTooltipState(),
        ) {
            IconButton(onClick = { scope.launch { searchBarState.animateToCollapsed() } }) {
                Icon(Icons.AutoMirrored.Default.ArrowBack, contentDescription = "Back")
            }
        }
    } else {
        Icon(Icons.Default.Search, contentDescription = null)
    }

@Composable
private fun SampleTrailingIcon() =
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = { PlainTooltip { Text("Mic") } },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = { /* doSomething() */ }) {
            Icon(imageVector = Icons.Default.Mic, contentDescription = "Mic")
        }
    }

@Composable
private fun SampleNavigationIcon(state: SearchBarState, isAnimated: Boolean = false) =
    AnimatedVisibility(
        visible = !isAnimated || state.targetValue == SearchBarValue.Collapsed,
        enter =
            slideIn(
                animationSpec = motionScheme.fastSpatialSpec(),
                initialOffset = { IntOffset(-it.width, 0) },
            ),
        exit =
            slideOut(
                animationSpec = tween(durationMillis = 150, delayMillis = 0),
                targetOffset = { IntOffset(-it.width, 0) },
            ),
    ) {
        TooltipBox(
            positionProvider =
                TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = { PlainTooltip { Text("Menu") } },
            state = rememberTooltipState(),
        ) {
            IconButton(onClick = { /* doSomething() */ }) {
                Icon(imageVector = Icons.Default.Menu, contentDescription = "Menu")
            }
        }
    }

@Composable
private fun SampleActions(state: SearchBarState, isAnimated: Boolean = false) =
    AnimatedVisibility(
        visible = !isAnimated || state.targetValue == SearchBarValue.Collapsed,
        enter =
            slideIn(
                animationSpec = motionScheme.fastSpatialSpec(),
                initialOffset = { IntOffset(it.width, 0) },
            ),
        exit =
            slideOut(
                animationSpec = tween(durationMillis = 150, delayMillis = 0),
                targetOffset = { IntOffset(it.width, 0) },
            ),
    ) {
        TooltipBox(
            positionProvider =
                TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = { PlainTooltip { Text("Account") } },
            state = rememberTooltipState(),
        ) {
            IconButton(onClick = { /* doSomething() */ }) {
                Icon(imageVector = Icons.Default.AccountCircle, contentDescription = "Account")
            }
        }
    }

// ---- each public search API by name ----

// The bars of a state and their expanded forms, the deprecated bars of a query, the states, the scroll behavior and
// SearchBarDefaults' members by Kotlin's names; what each gave handed to `seen`.
@Suppress("DEPRECATION")
@Composable
fun SearchBarsByName(form: Int, seen: (String) -> Unit) {
    val text = rememberTextFieldState()
    val state = rememberSearchBarState(
        initialValue = SearchBarValue.Collapsed,
        animationSpecForExpand = snap(),
        animationSpecForCollapse = snap(),
    )
    val contained = rememberContainedSearchBarState(initialValue = SearchBarValue.Collapsed)
    val withGap = rememberWithGapSearchBarState(initialValue = SearchBarValue.Collapsed)
    var expanded by remember { mutableStateOf(false) }
    var query by remember { mutableStateOf("") }
    val behavior = SearchBarDefaults.enterAlwaysSearchBarScrollBehavior(
        initialOffset = 0f,
        initialOffsetLimit = -Float.MAX_VALUE,
        initialContentOffset = 0f,
        canScroll = { true },
        snapAnimationSpec = spring(),
        flingAnimationSpec = rememberSplineBasedDecay(),
        reverseLayout = false,
    )
    val colors = SearchBarDefaults.colors(
        containerColor = Color.Red,
        dividerColor = Color.Blue,
        inputFieldColors = SearchBarDefaults.inputFieldColors(focusedTextColor = Color.Green),
    )
    val appBarColors = SearchBarDefaults.appBarWithSearchColors(
        searchBarColors = colors,
        scrolledSearchBarContainerColor = Color.Gray,
        appBarContainerColor = Color.White,
        scrolledAppBarContainerColor = Color.Black,
        appBarNavigationIconColor = Color.Cyan,
        appBarActionIconColor = Color.Magenta,
    )
    seen("colors ${colors.containerColor == Color.Red} ${colors.dividerColor == Color.Blue} ${colors.inputFieldColors.focusedTextColor == Color.Green}")
    seen("app bar ${appBarColors.searchBarColors == colors} ${appBarColors.appBarActionIconColor == Color.Magenta}")
    seen("contained ${SearchBarDefaults.containedColors(state = contained).containerColor == SearchBarDefaults.collapsedContainedSearchBarColor}")
    val copied = colors.copy(containerColor = Color.Yellow, inputFieldColors = colors.inputFieldColors)
    seen("copy ${copied.containerColor == Color.Yellow && copied.dividerColor == Color.Blue}")
    val made = SearchBarColors(containerColor = Color.Red, dividerColor = Color.Blue, inputFieldColors = colors.inputFieldColors)
    val old = SearchBarColors(containerColor = Color.Red, dividerColor = Color.Blue)
    seen("made ${made == colors} ${old.inputFieldColors.focusedTextColor == Color.Unspecified}")
    val appMade = AppBarWithSearchColors(
        searchBarColors = colors,
        scrolledSearchBarContainerColor = Color.Gray,
        appBarContainerColor = Color.White,
        scrolledAppBarContainerColor = Color.Black,
        appBarNavigationIconColor = Color.Cyan,
        appBarActionIconColor = Color.Magenta,
    )
    val appOld = AppBarWithSearchColors(
        searchBarColors = colors,
        appBarContainerColor = Color.White,
        appBarNavigationIconColor = Color.Cyan,
        appBarActionIconColor = Color.Magenta,
    )
    seen("app made ${appMade == appBarColors} ${appOld.scrolledAppBarContainerColor == Color.Unspecified}")
    seen(
        "defaults ${SearchBarDefaults.TonalElevation} ${SearchBarDefaults.ShadowElevation} ${SearchBarDefaults.Elevation} " +
            "${SearchBarDefaults.InputFieldHeight} ${SearchBarDefaults.dockedDropdownGapSize} " +
            "${SearchBarDefaults.AppBarContentPadding == PaddingValues(0.dp)} " +
            "${SearchBarDefaults.inputFieldShape == CircleShape} ${SearchBarDefaults.fullScreenShape == RectangleShape} " +
            "${SearchBarDefaults.dockedShape == MaterialTheme.shapes.extraLarge} " +
            "${SearchBarDefaults.dockedDropdownShape == RoundedCornerShape(12.dp)} " +
            "${SearchBarDefaults.dockedDropdownScrimColor != Color.Unspecified} " +
            "${SearchBarDefaults.fullScreenContainedSearchBarColor == MaterialTheme.colorScheme.surfaceContainerLow}"
    )
    val insets = SearchBarDefaults.windowInsets
    val fullScreenInsets = SearchBarDefaults.fullScreenWindowInsets
    seen("insets ${insets.getTop(LocalDensity.current)} ${fullScreenInsets.getTop(LocalDensity.current)}")
    seen("state ${state.currentValue} ${state.targetValue} ${state.progress} ${state.isAnimating} ${withGap.currentValue}")
    seen("behavior ${behavior.scrollOffset} ${behavior.contentOffset}")
    val spec: AnimationSpec<Float> = snap()
    val saved = rememberSaveable(
        saver = SearchBarState.Saver(
            animationSpecForExpand = spec,
            animationSpecForCollapse = spec,
            animationSpecForContentFadeIn = spec,
            animationSpecForContentFadeOut = spec,
        )
    ) {
        SearchBarState(
            initialValue = SearchBarValue.Expanded,
            animationSpecForExpand = spec,
            animationSpecForCollapse = spec,
            animationSpecForContentFadeIn = spec,
            animationSpecForContentFadeOut = spec,
        )
    }
    seen("made state ${saved.currentValue} ${saved.progress}")
    val field = @Composable {
        SearchBarDefaults.InputField(
            textFieldState = text,
            searchBarState = state,
            onSearch = { seen("searched $it") },
            modifier = Modifier.testTag("alan"),
            enabled = true,
            readOnly = false,
            textStyle = LocalTextStyle.current,
            placeholder = { Text("Ara") },
            leadingIcon = { Icon(Icons.Default.Search, contentDescription = null) },
            trailingIcon = null,
            prefix = null,
            suffix = null,
            inputTransformation = null,
            outputTransformation = null,
            scrollState = rememberScrollState(),
            shape = SearchBarDefaults.inputFieldShape,
            colors = SearchBarDefaults.inputFieldColors(),
            interactionSource = null,
            keyboardOptions = KeyboardOptions.Default,
            lineLimits = TextFieldLineLimits.SingleLine,
        )
    }
    Column {
        when (form) {
            0 -> {
                SearchBar(
                    state = state,
                    inputField = field,
                    modifier = Modifier.testTag("durum"),
                    shape = SearchBarDefaults.inputFieldShape,
                    colors = colors,
                    tonalElevation = SearchBarDefaults.TonalElevation,
                    shadowElevation = SearchBarDefaults.ShadowElevation,
                )
                ExpandedFullScreenSearchBar(
                    state = state,
                    inputField = field,
                    modifier = Modifier,
                    collapsedShape = SearchBarDefaults.inputFieldShape,
                    colors = colors,
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                    windowInsets = { SearchBarDefaults.fullScreenWindowInsets },
                    properties = DialogProperties(),
                ) {
                    Text("tam ekran", Modifier.weight(1f))
                }
            }
            1 -> {
                AppBarWithSearch(
                    state = state,
                    inputField = field,
                    modifier = Modifier.testTag("çubuk"),
                    navigationIcon = { Icon(Icons.Default.Menu, contentDescription = "Menü") },
                    actions = { Icon(Icons.Default.AccountCircle, contentDescription = "Hesap", Modifier.weight(1f)) },
                    shape = SearchBarDefaults.inputFieldShape,
                    colors = appBarColors,
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                    contentPadding = SearchBarDefaults.AppBarContentPadding,
                    windowInsets = SearchBarDefaults.windowInsets,
                    scrollBehavior = behavior,
                )
                TopSearchBar(
                    state = state,
                    inputField = field,
                    modifier = Modifier.testTag("üst"),
                    shape = SearchBarDefaults.inputFieldShape,
                    colors = colors,
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                    windowInsets = SearchBarDefaults.windowInsets,
                    scrollBehavior = null,
                )
                ExpandedDockedSearchBar(
                    state = state,
                    inputField = field,
                    modifier = Modifier,
                    shape = SearchBarDefaults.dockedShape,
                    colors = colors,
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                    properties = PopupProperties(focusable = true, clippingEnabled = false),
                ) {
                    Text("yerleşik", Modifier.weight(1f))
                }
            }
            2 -> {
                SearchBar(state = contained, inputField = {
                    SearchBarDefaults.InputField(textFieldState = text, searchBarState = contained, onSearch = {})
                })
                ExpandedFullScreenContainedSearchBar(
                    state = contained,
                    inputField = { SearchBarDefaults.InputField(textFieldState = text, searchBarState = contained, onSearch = {}) },
                    modifier = Modifier,
                    collapsedShape = SearchBarDefaults.inputFieldShape,
                    colors = SearchBarDefaults.containedColors(state = contained),
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                    windowInsets = { SearchBarDefaults.fullScreenWindowInsets },
                    properties = DialogProperties(),
                ) {
                    Text("içeren", Modifier.weight(1f))
                }
                SearchBar(state = withGap, inputField = {
                    SearchBarDefaults.InputField(textFieldState = text, searchBarState = withGap, onSearch = {})
                }, modifier = Modifier.testTag("aralıklı"))
                ExpandedDockedSearchBarWithGap(
                    state = withGap,
                    inputField = { SearchBarDefaults.InputField(textFieldState = text, searchBarState = withGap, onSearch = {}) },
                    modifier = Modifier,
                    shape = SearchBarDefaults.dockedShape,
                    dropdownShape = SearchBarDefaults.dockedDropdownShape,
                    dropdownGapSize = SearchBarDefaults.dockedDropdownGapSize,
                    dropdownScrimColor = SearchBarDefaults.dockedDropdownScrimColor,
                    colors = colors,
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                    properties = PopupProperties(focusable = true, clippingEnabled = false),
                ) {
                    Text("aralıklı sonuç", Modifier.weight(1f))
                }
            }
            3 -> {
                SearchBar(
                    inputField = {
                        SearchBarDefaults.InputField(
                            state = text,
                            onSearch = { seen("searched $it") },
                            expanded = expanded,
                            onExpandedChange = { expanded = it },
                            modifier = Modifier.testTag("alan"),
                            enabled = true,
                            readOnly = false,
                            textStyle = LocalTextStyle.current,
                            placeholder = { Text("Ara") },
                            leadingIcon = null,
                            trailingIcon = null,
                            prefix = null,
                            suffix = null,
                            inputTransformation = null,
                            outputTransformation = null,
                            scrollState = rememberScrollState(),
                            shape = SearchBarDefaults.inputFieldShape,
                            colors = SearchBarDefaults.inputFieldColors(),
                            interactionSource = null,
                        )
                    },
                    expanded = expanded,
                    onExpandedChange = { expanded = it },
                    modifier = Modifier.testTag("genişleyen"),
                    shape = SearchBarDefaults.inputFieldShape,
                    colors = colors,
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                    windowInsets = SearchBarDefaults.windowInsets,
                ) {
                    Text("öneri", Modifier.weight(1f))
                }
            }
            4 -> {
                DockedSearchBar(
                    inputField = {
                        SearchBarDefaults.InputField(
                            query = query,
                            onQueryChange = { query = it },
                            onSearch = { seen("searched $it") },
                            expanded = expanded,
                            onExpandedChange = { expanded = it },
                            modifier = Modifier.testTag("alan"),
                            enabled = true,
                            placeholder = { Text("Ara") },
                            leadingIcon = null,
                            trailingIcon = null,
                            colors = SearchBarDefaults.inputFieldColors(),
                            interactionSource = null,
                        )
                    },
                    expanded = expanded,
                    onExpandedChange = { expanded = it },
                    modifier = Modifier.testTag("yerleşik"),
                    shape = SearchBarDefaults.dockedShape,
                    colors = colors,
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                ) {
                    Text("öneri", Modifier.weight(1f))
                }
            }
            5 -> {
                SearchBar(
                    query = query,
                    onQueryChange = { query = it },
                    onSearch = { seen("searched $it") },
                    active = expanded,
                    onActiveChange = { expanded = it },
                    modifier = Modifier.testTag("eski"),
                    enabled = true,
                    placeholder = { Text("Ara") },
                    leadingIcon = null,
                    trailingIcon = null,
                    shape = SearchBarDefaults.inputFieldShape,
                    colors = colors,
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                    windowInsets = SearchBarDefaults.windowInsets,
                    interactionSource = null,
                ) {
                    Text("öneri", Modifier.weight(1f))
                }
            }
            else -> {
                DockedSearchBar(
                    query = query,
                    onQueryChange = { query = it },
                    onSearch = { seen("searched $it") },
                    active = expanded,
                    onActiveChange = { expanded = it },
                    modifier = Modifier.testTag("eski yerleşik"),
                    enabled = true,
                    placeholder = { Text("Ara") },
                    leadingIcon = null,
                    trailingIcon = null,
                    shape = SearchBarDefaults.dockedShape,
                    colors = colors,
                    tonalElevation = 0.dp,
                    shadowElevation = 0.dp,
                    interactionSource = null,
                ) {
                    Text("öneri", Modifier.weight(1f))
                }
            }
        }
    }
}
