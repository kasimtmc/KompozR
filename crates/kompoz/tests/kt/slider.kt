// Material's sliders as Compose writes them: the slider half of the C++ Kompoz's Expressive source
// (kompozable_tests.cpp expressive_buttons_and_sliders_as_compose_writes_them) as M15 accepts it, and material3 1.5's
// SliderSamples. (Waiting: VerticalSliderSample and VerticalCenteredSliderSample for a snapshot state of a Job, see
// VerticalSliders.) SliderWithTrackIconsSample draws its icons with Painter's member extension `DrawScope.draw(size,
// alpha, colorFilter)`, both receivers implicit: `with(painter) { draw(…) }` in a DrawScope lambda.

// A slider's valueRange, its thumb slot with a Label, a track lambda; a RangeSlider's range value and steps;
// rememberSliderState, rememberRangeSliderState; `price.start`, `.endInclusive`, `stars.value.roundToInt()`.
@Composable
fun ExpressiveSliders() {
    var bold by remember { mutableStateOf(false) }
    var volume by remember { mutableStateOf(40f) }
    var price by remember { mutableStateOf(200f..800f) }
    val source = remember { MutableInteractionSource() }
    var finished: Int? by remember { mutableStateOf(null) }
    val stars = rememberSliderState(value = 3f, steps = 3, valueRange = 1f..5f)
    val hours = rememberRangeSliderState(activeRangeStart = 9f, activeRangeEnd = 17f, steps = 23, valueRange = 0f..24f)
    Column {
        Checkbox(checked = bold, onCheckedChange = { bold = it }, modifier = Modifier.testTag("kalın"))
        // (C++: `track = if (bold) { { s -> SliderDefaults.Track(s) } } else null`; M15's track isn't nullable, its
        // drawStopIndicator is: the lambda or null goes there.)
        Slider(
            value = volume,
            onValueChange = { volume = it },
            modifier = Modifier.testTag("ses"),
            onValueChangeFinished = { finished = (finished ?: 0) + 1 },
            valueRange = 0f..100f,
            interactionSource = source,
            thumb = { s ->
                Label(label = { PlainTooltip { Text("%.0f".format(volume), modifier = Modifier.testTag("değer")) } }, interactionSource = source) {
                    SliderDefaults.Thumb(interactionSource = source, sliderState = s)
                }
            },
            track = { s ->
                SliderDefaults.Track(
                    s,
                    drawStopIndicator = if (bold) { { drawCircle(Color.Red, radius = 3.dp.toPx(), center = it) } } else null,
                )
            },
        )
        RangeSlider(value = price, onValueChange = { price = it }, modifier = Modifier.testTag("fiyat"), valueRange = 0f..1000f, steps = 19)
        Slider(state = stars, modifier = Modifier.testTag("yıldız"))
        // (the track's overload by its first argument's type: a RangeSliderState's)
        RangeSlider(state = hours, modifier = Modifier.testTag("saat"), track = { SliderDefaults.Track(it) })
        Text("bitti: ${finished ?: 0}")
        val inPrice = 500f in price
        Text(
            "${price.start.roundToInt()}–${price.endInclusive.roundToInt()} · ${stars.value.roundToInt()} · " +
                "${hours.activeRangeEnd.roundToInt()} · ${volume.roundToInt()} · ${if (inPrice) "içinde" else "dışında"}"
        )
    }
}

// material3's SliderSamples.

@Composable
fun SliderSample() {
    var sliderPosition by rememberSaveable { mutableStateOf(0f) }
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Text(text = "%.2f".format(sliderPosition))
        Slider(value = sliderPosition, onValueChange = { sliderPosition = it })
    }
}

@Composable
fun LegacySliderSample() {
    var sliderPosition by rememberSaveable { mutableStateOf(0f) }
    val interactionSource = remember { MutableInteractionSource() }
    val trackHeight = 4.dp
    val thumbSize = DpSize(20.dp, 20.dp)
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Text(text = "%.2f".format(sliderPosition))
        Slider(
            interactionSource = interactionSource,
            modifier = Modifier.requiredSizeIn(minWidth = thumbSize.width, minHeight = trackHeight),
            value = sliderPosition,
            onValueChange = { sliderPosition = it },
            thumb = {
                val modifier =
                    Modifier.size(thumbSize)
                        .shadow(1.dp, CircleShape, clip = false)
                        .indication(
                            interactionSource = interactionSource,
                            indication = ripple(bounded = false, radius = 20.dp),
                        )
                SliderDefaults.Thumb(interactionSource = interactionSource, modifier = modifier)
            },
            track = {
                val modifier = Modifier.height(trackHeight)
                SliderDefaults.Track(
                    sliderState = it,
                    modifier = modifier,
                    thumbTrackGapSize = 0.dp,
                    trackInsideCornerSize = 0.dp,
                    drawStopIndicator = null,
                )
            },
        )
    }
}

@Composable
fun StepsSliderSample() {
    val sliderState =
        rememberSliderState(
            // Only allow multiples of 10. Excluding the endpoints of `valueRange`,
            // there are 9 steps (10, 20, ..., 90).
            steps = 9,
            valueRange = 0f..100f,
            onValueChangeFinished = {
                // launch some business logic update with the state you hold
                // viewModel.updateSelectedSliderValue(sliderPosition)
            },
        )
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Text(text = "%.2f".format(sliderState.value))
        Slider(state = sliderState)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SliderWithCustomThumbSample() {
    var sliderPosition by rememberSaveable { mutableStateOf(0f) }
    val interactionSource: MutableInteractionSource = remember { MutableInteractionSource() }
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Slider(
            value = sliderPosition,
            onValueChange = { sliderPosition = it },
            valueRange = 0f..100f,
            interactionSource = interactionSource,
            onValueChangeFinished = {
                // launch some business logic update with the state you hold
                // viewModel.updateSelectedSliderValue(sliderPosition)
            },
            thumb = {
                Label(
                    label = {
                        PlainTooltip(modifier = Modifier.sizeIn(45.dp, 25.dp).wrapContentWidth()) {
                            Text("%.2f".format(sliderPosition))
                        }
                    },
                    interactionSource = interactionSource,
                ) {
                    Icon(
                        imageVector = Icons.Filled.Favorite,
                        contentDescription = null,
                        modifier = Modifier.size(ButtonDefaults.IconSize),
                        tint = Color.Red,
                    )
                }
            },
        )
    }
}

@Composable
fun SliderWithCustomTrackAndThumbSample() {
    val sliderState =
        rememberSliderState(
            valueRange = 0f..100f,
            onValueChangeFinished = {
                // launch some business logic update with the state you hold
                // viewModel.updateSelectedSliderValue(sliderPosition)
            },
        )
    val interactionSource = remember { MutableInteractionSource() }
    val colors = SliderDefaults.colors(thumbColor = Color.Red, activeTrackColor = Color.Red)
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Text(text = "%.2f".format(sliderState.value))
        Slider(
            state = sliderState,
            interactionSource = interactionSource,
            thumb = {
                SliderDefaults.Thumb(interactionSource = interactionSource, colors = colors)
            },
            track = { SliderDefaults.Track(colors = colors, sliderState = sliderState) },
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SliderWithTrackIconsSample() {
    val sliderState =
        rememberSliderState(
            valueRange = 0f..100f,
            onValueChangeFinished = {
                // launch some business logic update with the state you hold
                // viewModel.updateSelectedSliderValue(sliderPosition)
            },
        )
    val interactionSource = remember { MutableInteractionSource() }
    val startIcon = rememberVectorPainter(Icons.Filled.MusicNote)
    val endIcon = rememberVectorPainter(Icons.Filled.MusicOff)
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Text(text = "%.2f".format(sliderState.value))
        Slider(
            state = sliderState,
            interactionSource = interactionSource,
            track = {
                val iconSize = DpSize(20.dp, 20.dp)
                val iconPadding = 10.dp
                val thumbTrackGapSize = 6.dp
                val activeIconColor = SliderDefaults.colors().activeTickColor
                val inactiveIconColor = SliderDefaults.colors().inactiveTickColor
                val trackIconStart: DrawScope.(Offset, Color) -> Unit = { offset, color ->
                    translate(offset.x + iconPadding.toPx(), offset.y) {
                        with(startIcon) {
                            draw(iconSize.toSize(), colorFilter = ColorFilter.tint(color))
                        }
                    }
                }
                val trackIconEnd: DrawScope.(Offset, Color) -> Unit = { offset, color ->
                    translate(offset.x - iconPadding.toPx() - iconSize.toSize().width, offset.y) {
                        with(endIcon) {
                            draw(iconSize.toSize(), colorFilter = ColorFilter.tint(color))
                        }
                    }
                }
                SliderDefaults.Track(
                    sliderState = sliderState,
                    modifier =
                        Modifier.height(36.dp).drawWithContent {
                            drawContent()

                            val yOffset = size.height / 2 - iconSize.toSize().height / 2
                            val activeTrackStart = 0f
                            val activeTrackEnd =
                                size.width * sliderState.coercedValueAsFraction -
                                    thumbTrackGapSize.toPx()
                            val inactiveTrackStart = activeTrackEnd + thumbTrackGapSize.toPx() * 2
                            val inactiveTrackEnd = size.width

                            val activeTrackWidth = activeTrackEnd - activeTrackStart
                            val inactiveTrackWidth = inactiveTrackEnd - inactiveTrackStart
                            if (
                                iconSize.toSize().width < activeTrackWidth - iconPadding.toPx() * 2
                            ) {
                                trackIconStart(Offset(activeTrackStart, yOffset), activeIconColor)
                                trackIconEnd(Offset(activeTrackEnd, yOffset), activeIconColor)
                            }
                            if (
                                iconSize.toSize().width <
                                    inactiveTrackWidth - iconPadding.toPx() * 2
                            ) {
                                trackIconStart(
                                    Offset(inactiveTrackStart, yOffset),
                                    inactiveIconColor,
                                )
                                trackIconEnd(Offset(inactiveTrackEnd, yOffset), inactiveIconColor)
                            }
                        },
                    trackCornerSize = 12.dp,
                    drawStopIndicator = null,
                    thumbTrackGapSize = thumbTrackGapSize,
                )
            },
        )
    }
}

@Composable
fun CenteredSliderSample() {
    val sliderState =
        rememberSliderState(
            valueRange = -50f..50f,
            onValueChangeFinished = {
                // launch some business logic update with the state you hold
                // viewModel.updateSelectedSliderValue(sliderPosition)
            },
        )
    val interactionSource = remember { MutableInteractionSource() }
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Text(text = "%.2f".format(sliderState.value))
        Slider(
            state = sliderState,
            interactionSource = interactionSource,
            thumb = { SliderDefaults.Thumb(interactionSource = interactionSource) },
            track = { SliderDefaults.CenteredTrack(sliderState = sliderState) },
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Preview
@Sampled
@Composable
fun VerticalSliderSample() {
    val coroutineScope = rememberCoroutineScope()
    val sliderState =
        rememberSliderState(
            // Only allow multiples of 10. Excluding the endpoints of `valueRange`,
            // there are 9 steps (10, 20, ..., 90).
            steps = 9,
            valueRange = 0f..100f,
        )
    val snapAnimationSpec = MaterialTheme.motionScheme.fastEffectsSpec<Float>()
    var currentValue by rememberSaveable { mutableFloatStateOf(sliderState.value) }
    var animateJob: Job? by remember { mutableStateOf(null) }
    sliderState.shouldAutoSnap = false
    sliderState.onValueChange = { newValue ->
        currentValue = newValue
        // only update the sliderState instantly if dragging
        if (sliderState.isDragging) {
            animateJob?.cancel()
            sliderState.value = newValue
        }
    }
    sliderState.onValueChangeFinished = {
        animateJob =
            coroutineScope.launch {
                animate(
                    initialValue = sliderState.value,
                    targetValue = currentValue,
                    animationSpec = snapAnimationSpec,
                ) { value, _ ->
                    sliderState.value = value
                }
            }
    }
    val interactionSource = remember { MutableInteractionSource() }
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "%.2f".format(sliderState.value),
        )
        Spacer(Modifier.height(16.dp))
        VerticalSlider(
            state = sliderState,
            modifier =
                Modifier.height(300.dp)
                    .align(Alignment.CenterHorizontally)
                    .progressSemantics(
                        currentValue,
                        sliderState.valueRange.start..sliderState.valueRange.endInclusive,
                        sliderState.steps,
                    ),
            interactionSource = interactionSource,
            track = {
                SliderDefaults.Track(
                    sliderState = sliderState,
                    modifier = Modifier.width(36.dp),
                    trackCornerSize = 12.dp,
                )
            },
            reverseDirection = true,
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Preview
@Sampled
@Composable
fun VerticalCenteredSliderSample() {
    val coroutineScope = rememberCoroutineScope()
    val sliderState =
        rememberSliderState(
            // Only allow multiples of 10. Excluding the endpoints of `valueRange`,
            // there are 9 steps (10, 20, ..., 90).
            steps = 9,
            valueRange = -50f..50f,
        )
    val snapAnimationSpec = MaterialTheme.motionScheme.fastEffectsSpec<Float>()
    var currentValue by rememberSaveable { mutableFloatStateOf(sliderState.value) }
    var animateJob: Job? by remember { mutableStateOf(null) }
    sliderState.shouldAutoSnap = false
    sliderState.onValueChange = { newValue ->
        currentValue = newValue
        // only update the sliderState instantly if dragging
        if (sliderState.isDragging) {
            animateJob?.cancel()
            sliderState.value = newValue
        }
    }
    sliderState.onValueChangeFinished = {
        animateJob =
            coroutineScope.launch {
                animate(
                    initialValue = sliderState.value,
                    targetValue = currentValue,
                    animationSpec = snapAnimationSpec,
                ) { value, _ ->
                    sliderState.value = value
                }
            }
    }
    val interactionSource = remember { MutableInteractionSource() }
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "%.2f".format(sliderState.value),
        )
        Spacer(Modifier.height(16.dp))
        VerticalSlider(
            state = sliderState,
            modifier =
                Modifier.height(300.dp)
                    .align(Alignment.CenterHorizontally)
                    .progressSemantics(
                        currentValue,
                        sliderState.valueRange.start..sliderState.valueRange.endInclusive,
                        sliderState.steps,
                    ),
            interactionSource = interactionSource,
            track = { SliderDefaults.CenteredTrack(sliderState = sliderState) },
            reverseDirection = true,
        )
    }
}


@Composable
fun RangeSliderSample() {
    val rangeSliderState =
        rememberRangeSliderState(
            0f,
            100f,
            valueRange = 0f..100f,
            onValueChangeFinished = {
                // launch some business logic update with the state you hold
                // viewModel.updateSelectedSliderValue(sliderPosition)
            },
        )
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        val rangeStart = "%.2f".format(rangeSliderState.activeRangeStart)
        val rangeEnd = "%.2f".format(rangeSliderState.activeRangeEnd)
        Text(text = "$rangeStart .. $rangeEnd")
        RangeSlider(state = rangeSliderState)
    }
}

@Composable
fun LegacyRangeSliderSample() {
    val rangeSliderState =
        rememberRangeSliderState(
            0f,
            100f,
            valueRange = 0f..100f,
            onValueChangeFinished = {
                // launch some business logic update with the state you hold
                // viewModel.updateSelectedSliderValue(sliderPosition)
            },
        )
    val startInteractionSource = remember { MutableInteractionSource() }
    val endInteractionSource = remember { MutableInteractionSource() }
    val trackHeight = 4.dp
    val thumbSize = DpSize(20.dp, 20.dp)
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        val rangeStart = "%.2f".format(rangeSliderState.activeRangeStart)
        val rangeEnd = "%.2f".format(rangeSliderState.activeRangeEnd)
        Text(text = "$rangeStart .. $rangeEnd")
        RangeSlider(
            state = rangeSliderState,
            startInteractionSource = startInteractionSource,
            endInteractionSource = endInteractionSource,
            modifier = Modifier.requiredSizeIn(minWidth = thumbSize.width, minHeight = trackHeight),
            startThumb = {
                val modifier =
                    Modifier.size(thumbSize)
                        .shadow(1.dp, CircleShape, clip = false)
                        .indication(
                            interactionSource = startInteractionSource,
                            indication = ripple(bounded = false, radius = 20.dp),
                        )
                SliderDefaults.Thumb(
                    interactionSource = startInteractionSource,
                    modifier = modifier,
                )
            },
            endThumb = {
                val modifier =
                    Modifier.size(thumbSize)
                        .shadow(1.dp, CircleShape, clip = false)
                        .indication(
                            interactionSource = endInteractionSource,
                            indication = ripple(bounded = false, radius = 20.dp),
                        )
                SliderDefaults.Thumb(interactionSource = endInteractionSource, modifier = modifier)
            },
            track = {
                val modifier = Modifier.height(trackHeight)
                SliderDefaults.Track(
                    rangeSliderState = it,
                    modifier = modifier,
                    thumbTrackGapSize = 0.dp,
                    trackInsideCornerSize = 0.dp,
                    drawStopIndicator = null,
                )
            },
        )
    }
}

@Composable
fun StepRangeSliderSample() {
    val rangeSliderState =
        rememberRangeSliderState(
            0f,
            100f,
            valueRange = 0f..100f,
            onValueChangeFinished = {
                // launch some business logic update with the state you hold
                // viewModel.updateSelectedSliderValue(sliderPosition)
            },
            // Only allow multiples of 10. Excluding the endpoints of `valueRange`,
            // there are 9 steps (10, 20, ..., 90).
            steps = 9,
        )
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        val rangeStart = rangeSliderState.activeRangeStart.roundToInt()
        val rangeEnd = rangeSliderState.activeRangeEnd.roundToInt()
        Text(text = "$rangeStart .. $rangeEnd")
        RangeSlider(state = rangeSliderState)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RangeSliderWithCustomComponents() {
    val rangeSliderState =
        rememberRangeSliderState(
            0f,
            100f,
            valueRange = 0f..100f,
            onValueChangeFinished = {
                // launch some business logic update with the state you hold
                // viewModel.updateSelectedSliderValue(sliderPosition)
            },
        )
    val startInteractionSource = remember { MutableInteractionSource() }
    val endInteractionSource = remember { MutableInteractionSource() }
    val startThumbAndTrackColors =
        SliderDefaults.colors(thumbColor = Color.Blue, activeTrackColor = Color.Red)
    val endThumbColors = SliderDefaults.colors(thumbColor = Color.Green)
    Column(modifier = Modifier.padding(horizontal = 16.dp)) {
        RangeSlider(
            state = rangeSliderState,
            startInteractionSource = startInteractionSource,
            endInteractionSource = endInteractionSource,
            startThumb = {
                Label(
                    label = {
                        PlainTooltip(modifier = Modifier.sizeIn(45.dp, 25.dp).wrapContentWidth()) {
                            Text("%.2f".format(rangeSliderState.activeRangeStart))
                        }
                    },
                    interactionSource = startInteractionSource,
                ) {
                    SliderDefaults.Thumb(
                        interactionSource = startInteractionSource,
                        colors = startThumbAndTrackColors,
                    )
                }
            },
            endThumb = {
                Label(
                    label = {
                        PlainTooltip(
                            modifier = Modifier.requiredSize(45.dp, 25.dp).wrapContentWidth()
                        ) {
                            Text("%.2f".format(rangeSliderState.activeRangeEnd))
                        }
                    },
                    interactionSource = endInteractionSource,
                ) {
                    SliderDefaults.Thumb(
                        interactionSource = endInteractionSource,
                        colors = endThumbColors,
                    )
                }
            },
            track = { rangeSliderState ->
                SliderDefaults.Track(
                    colors = startThumbAndTrackColors,
                    rangeSliderState = rangeSliderState,
                )
            },
        )
    }
}
