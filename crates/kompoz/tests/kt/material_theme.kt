// Material's theme as Compose's samples write it (material3 1.5's ThemeSamples, SurfaceSamples, ColorSchemeSamples,
// TextSamples), on the desktop: the system's dynamic colors where the host has them, its dark theme.

@Composable
fun MaterialThemeSample() {
    val isDarkTheme = isSystemInDarkTheme()
    val supportsDynamicColor = isDynamicColorAvailable()
    val lightColorScheme = lightColorScheme(primary = Color(0xFF1EB980))
    val darkColorScheme = darkColorScheme(primary = Color(0xFF66ffc7))
    val colorScheme =
        when {
            supportsDynamicColor && isDarkTheme -> dynamicDarkColorScheme()
            supportsDynamicColor && !isDarkTheme -> dynamicLightColorScheme()
            isDarkTheme -> darkColorScheme
            else -> lightColorScheme
        }
    val typography =
        Typography(
            displaySmall = TextStyle(fontWeight = FontWeight.W100, fontSize = 96.sp),
            labelLarge = TextStyle(fontWeight = FontWeight.W600, fontSize = 14.sp),
        )
    val shapes = Shapes(extraSmall = RoundedCornerShape(3.0.dp), small = RoundedCornerShape(6.0.dp))
    MaterialTheme(colorScheme = colorScheme, typography = typography, shapes = shapes) {
        val currentTheme = if (!isSystemInDarkTheme()) "light" else "dark"
        Row {
            Icon(Icons.Filled.Favorite, contentDescription = "Localized Description")
            Text("Text style and color from $currentTheme theme", style = MaterialTheme.typography.labelLarge, modifier = Modifier.testTag("label"))
        }
    }
}

@Composable
fun MaterialExpressiveThemeSample() {
    val isDarkTheme = isSystemInDarkTheme()
    val darkColorScheme = darkColorScheme(primary = Color(0xFF66ffc7))
    val colorScheme = if (isDarkTheme) darkColorScheme else expressiveLightColorScheme()
    val shapes = Shapes(largeIncreased = RoundedCornerShape(36.0.dp))
    MaterialExpressiveTheme(colorScheme = colorScheme, shapes = shapes) {
        Text("increased ${MaterialTheme.shapes.largeIncreased == RoundedCornerShape(36.0.dp)}")
        Text("expressive ${MaterialTheme.motionScheme == MotionScheme.expressive()}")
    }
}

@Composable
fun MyMaterialTheme(content: @Composable () -> Unit) {
    MaterialExpressiveTheme(
        colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else expressiveLightColorScheme()
    ) {
        content()
    }
}

@Composable
fun ThemeColorSample() {
    val colorScheme = MaterialTheme.colorScheme
    Box(Modifier.size(20.dp).aspectRatio(1f).fillMaxSize().background(color = colorScheme.primary).testTag("primary"))
}

@Composable
fun ThemeTextStyleSample() {
    val typography = MaterialTheme.typography
    Text(text = "Headline small styled text", style = typography.headlineSmall, modifier = Modifier.testTag("headline"))
}

@Composable
fun SurfaceSample() {
    Surface { Text("Text on Surface") }
}

@Composable
fun ClickableSurfaceSample() {
    var count by remember { mutableStateOf(0) }
    Surface(onClick = { count++ }) { Text("Clickable Surface. Count: $count") }
}

@Composable
fun SelectableSurfaceSample() {
    var selected by remember { mutableStateOf(false) }
    Surface(selected = selected, onClick = { selected = !selected }) {
        Text(text = if (selected) "Selected" else "Not Selected", textAlign = TextAlign.Center)
    }
}

@Composable
fun ToggleableSurfaceSample() {
    var checked by remember { mutableStateOf(false) }
    Surface(
        checked = checked,
        onCheckedChange = { checked = !checked },
        color =
            if (checked) {
                MaterialTheme.colorScheme.surfaceVariant
            } else {
                MaterialTheme.colorScheme.surface
            },
    ) {
        Text(text = if (checked) "ON" else "OFF", textAlign = TextAlign.Center)
    }
}
