// Material's progress indicators as Compose writes them: the progress lines of the C++ Kompoz's MaterialForms and
// GraphicsForms (kompozable_tests.cpp material_forms_as_compose_writes_them, graphics_forms_as_compose_writes_them:
// the rest of those sources is other packages'), material3 1.5's ProgressIndicatorSamples (the wavy ones too); the
// loading indicators of the C++'s Containers and GraphicsForms and material3 1.5's LoadingIndicatorSamples.

// MaterialForms' progress: `progress = { … }`.
@Composable
fun MaterialFormsProgress() {
    Column {
        LinearProgressIndicator(progress = { 0.4f }, modifier = Modifier.testTag("bar"))
    }
}

// GraphicsForms' wavy indicators: a lambda amplitude, a stop size; an endless one with its track's color.
@Composable
fun GraphicsFormsProgress() {
    Column {
        LinearWavyProgressIndicator(progress = { 0.5f }, modifier = Modifier.width(100.dp).testTag("dalga"), amplitude = { 0.5f }, stopSize = 0.dp)
        CircularWavyProgressIndicator(trackColor = Color.Gray)
    }
}

// GraphicsForms' heart: a material shape (`MaterialShapes.Heart`, an object's member) as a background's shape; one turned.
@Composable
fun GraphicsFormsShapes() {
    Column {
        Box(modifier = Modifier.size(20.dp).background(Color.Red, MaterialShapes.Heart.toShape()).testTag("kalp"))
        Box(modifier = Modifier.size(20.dp).clip(MaterialShapes.Cookie9Sided.toShape(startAngle = 90)).background(Color.Blue).testTag("kurabiye"))
    }
}

// ---- ProgressIndicatorSamples ----------------------------------------------------------------------------------

@Composable
fun LinearProgressIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0.1f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec,
        )

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        LinearProgressIndicator(progress = { animatedProgress })
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

@Composable
fun LinearWavyProgressIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0.1f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec,
        )

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        LinearWavyProgressIndicator(progress = { animatedProgress })
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

@Composable
fun LinearThickWavyProgressIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0.1f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec,
        )

    val thickStrokeWidth = with(LocalDensity.current) { 8.dp.toPx() }
    val thickStroke =
        remember(thickStrokeWidth) { Stroke(width = thickStrokeWidth, cap = StrokeCap.Round) }

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        LinearWavyProgressIndicator(
            progress = { animatedProgress },
            // Thick height is slightly higher than the
            // WavyProgressIndicatorDefaults.LinearContainerHeight default
            modifier = Modifier.height(14.dp),
            stroke = thickStroke,
            trackStroke = thickStroke,
        )
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

@Composable
fun IndeterminateLinearProgressIndicatorSample() {
    Column(horizontalAlignment = Alignment.CenterHorizontally) { LinearProgressIndicator() }
}

@Composable
fun IndeterminateLinearWavyProgressIndicatorSample() {
    Column(horizontalAlignment = Alignment.CenterHorizontally) { LinearWavyProgressIndicator() }
}

@Composable
fun CircularProgressIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0.1f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec,
        )

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        CircularProgressIndicator(progress = { animatedProgress })
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

@Composable
fun CircularWavyProgressIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0.1f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec,
        )

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        CircularWavyProgressIndicator(progress = { animatedProgress })
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

@Composable
fun CircularThickWavyProgressIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0.1f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec,
        )
    val thickStrokeWidth = with(LocalDensity.current) { 8.dp.toPx() }
    val thickStroke =
        remember(thickStrokeWidth) { Stroke(width = thickStrokeWidth, cap = StrokeCap.Round) }
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        CircularWavyProgressIndicator(
            progress = { animatedProgress },
            // Thick size is slightly larger than the
            // WavyProgressIndicatorDefaults.CircularContainerSize default
            modifier = Modifier.size(52.dp), // Thick size is slightly larger than the default
            stroke = thickStroke,
            trackStroke = thickStroke,
        )
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

@Composable
fun IndeterminateCircularProgressIndicatorSample() {
    Column(horizontalAlignment = Alignment.CenterHorizontally) { CircularProgressIndicator() }
}

@Composable
fun IndeterminateCircularWavyProgressIndicatorSample() {
    Column(horizontalAlignment = Alignment.CenterHorizontally) { CircularWavyProgressIndicator() }
}

@Composable
fun LegacyLinearProgressIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0.1f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec,
        )

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        LinearProgressIndicator(
            progress = { animatedProgress },
            trackColor = MaterialTheme.colorScheme.surfaceVariant,
            strokeCap = StrokeCap.Butt,
            gapSize = 0.dp,
            drawStopIndicator = {},
        )
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

@Composable
fun LegacyIndeterminateLinearProgressIndicatorSample() {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        LinearProgressIndicator(
            trackColor = MaterialTheme.colorScheme.surfaceVariant,
            strokeCap = StrokeCap.Butt,
            gapSize = 0.dp,
        )
    }
}

@Composable
fun LegacyCircularProgressIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0.1f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec,
        )

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        CircularProgressIndicator(
            progress = { animatedProgress },
            trackColor = Color.Transparent,
            strokeCap = StrokeCap.Butt,
            gapSize = 0.dp,
        )
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

@Composable
fun LegacyIndeterminateCircularProgressIndicatorSample() {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        CircularProgressIndicator(strokeCap = StrokeCap.Butt)
    }
}

// ---- The deprecated value forms and the defaults by name -----------------------------------------------------

// (Compose's deprecated `progress: Float` overloads, still callable; a stop indicator drawn by the defaults' function;
// the wavy ones' strokes and wavelengths by name.)
@Composable
fun ProgressByName() {
    Column {
        LinearProgressIndicator(progress = 0.25f, modifier = Modifier.testTag("eski"))
        CircularProgressIndicator(progress = 0.75f, modifier = Modifier.testTag("eski halka"), strokeWidth = 2.dp)
        LinearProgressIndicator(
            progress = { 0.5f },
            modifier = Modifier.width(200.dp).testTag("durak"),
            drawStopIndicator = {
                ProgressIndicatorDefaults.drawStopIndicator(
                    drawScope = this,
                    stopSize = ProgressIndicatorDefaults.LinearTrackStopIndicatorSize,
                    color = Color.Red,
                    strokeCap = StrokeCap.Butt,
                )
            },
        )
        LinearWavyProgressIndicator(
            modifier = Modifier.testTag("belirsiz dalga"),
            stroke = WavyProgressIndicatorDefaults.linearIndicatorStroke,
            amplitude = 0.5f,
            wavelength = WavyProgressIndicatorDefaults.LinearIndeterminateWavelength,
            waveSpeed = 0.dp,
        )
        CircularWavyProgressIndicator(
            progress = { 0.5f },
            modifier = Modifier.testTag("dalgalı halka"),
            amplitude = WavyProgressIndicatorDefaults.indicatorAmplitude,
            wavelength = WavyProgressIndicatorDefaults.CircularWavelength,
            gapSize = WavyProgressIndicatorDefaults.CircularIndicatorTrackGapSize,
        )
    }
}

// ---- Loading indicators -----------------------------------------------------------------------------------------

// Containers' loading indicators (kompozable_tests.cpp containers_as_compose_writes_them: the rest of that source is
// other packages'): an endless one sized by its modifier, a determinate one by name.
@Composable
fun ContainersLoadingIndicators() {
    Column {
        LoadingIndicator(modifier = Modifier.size(48.dp).testTag("yükleniyor"))
        // (the C++ had `progress = 0.5f`: M15's progress is a lambda)
        LoadingIndicator(progress = { 0.5f }, modifier = Modifier.testTag("yarım"))
    }
}

// GraphicsForms' contained loading indicator (graphics_forms_as_compose_writes_them).
@Composable
fun GraphicsFormsLoadingIndicator() {
    Column {
        ContainedLoadingIndicator(progress = { 0.3f }, modifier = Modifier.testTag("kap"), containerColor = Color.Yellow)
    }
}

// ---- LoadingIndicatorSamples (LoadingIndicatorPullToRefreshSample, a Scaffold, is in swipe_pull.kt) --------------

@Composable
fun LoadingIndicatorSample() {
    Column(horizontalAlignment = Alignment.CenterHorizontally) { LoadingIndicator() }
}

@Composable
fun ContainedLoadingIndicatorSample() {
    Column(horizontalAlignment = Alignment.CenterHorizontally) { ContainedLoadingIndicator() }
}

@Composable
fun DeterminateLoadingIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec =
                spring(
                    dampingRatio = Spring.DampingRatioNoBouncy,
                    stiffness = Spring.StiffnessVeryLow,
                    visibilityThreshold = 1 / 1000f,
                ),
        )

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        LoadingIndicator(progress = { animatedProgress })
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set loading progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

@Composable
fun DeterminateContainedLoadingIndicatorSample() {
    var progress by remember { mutableFloatStateOf(0f) }
    val animatedProgress by
        animateFloatAsState(
            targetValue = progress,
            animationSpec =
                spring(
                    dampingRatio = Spring.DampingRatioNoBouncy,
                    stiffness = Spring.StiffnessVeryLow,
                    visibilityThreshold = 1 / 1000f,
                ),
        )

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        ContainedLoadingIndicator(progress = { animatedProgress })
        Spacer(Modifier.requiredHeight(30.dp))
        Text("Set loading progress:")
        Slider(
            modifier = Modifier.width(300.dp),
            value = progress,
            valueRange = 0f..1f,
            onValueChange = { progress = it },
        )
    }
}

// The loading indicators by name: colors, a container's shape, polygons (the defaults' lists, a list of some of
// them).
@Composable
fun LoadingIndicatorsByName() {
    val shapes = LoadingIndicatorDefaults.IndeterminateIndicatorPolygons
    Column {
        LoadingIndicator(
            progress = { 1f },
            modifier = Modifier.testTag("patlama"),
            color = Color.Red,
            polygons = LoadingIndicatorDefaults.DeterminateIndicatorPolygons,
        )
        LoadingIndicator(modifier = Modifier.testTag("dönen"), color = Color.Magenta, polygons = listOf(shapes[2], shapes[3]))
        ContainedLoadingIndicator(
            modifier = Modifier.testTag("kare"),
            containerColor = Color.Blue,
            indicatorColor = Color.Green,
            containerShape = RoundedCornerShape(8.dp),
            polygons = shapes,
        )
        Text("${LoadingIndicatorDefaults.ContainerWidth} ${LoadingIndicatorDefaults.IndicatorSize} ${shapes.size}")
    }
}
