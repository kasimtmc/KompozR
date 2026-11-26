// What the showcase's parts share (the C++ showcase's showcase.hpp and common.cpp): a section card, a desktop note,
// the theme animation, test colors, and the switch that outlines what recomposes.

// The screens, as type-safe routes (named, so the addresses read "item/42", "list/all"): the C++ showcase's
// `namespace routes` (a package has no namespace of its own: `Home`, not the C++'s `routes::Home()`).
@Serializable @SerialName("home") object Home

// The list and its items: a nested graph.
@Serializable @SerialName("list") object ListGraph

@Serializable @SerialName("list/all") object ListAll

@Serializable @SerialName("item") data class Item(val n: Int)

@Serializable @SerialName("components") object Components

@Serializable @SerialName("material") object Material

// BottomSheetScaffold over a map.
@Serializable @SerialName("places") object Places

// Whether the recompose highlighter is on (the eye in the app bar). The C++ showcase's `traced` modifier, which
// outlines a node with the highlighter while it is on, is used by the screens that come with stage 16 (and the
// highlighter, Compose's recomposeHighlighter snippet, with them).
val LocalShowRekompozitions = compositionLocalOf { false }

// Where the theme's colors come from: the app's own scheme, the system's accent color, or the
// wallpaper. The library offers all three; the app (here its user) picks.
enum class ColorSource { App, System, Wallpaper }

// Every color animates when the theme changes, each its own animation (their calls in a fixed order).
@Composable
fun animateColorScheme(t: ColorScheme): ColorScheme {
    val spec = tween<Color>(400)
    return t.copy(
        primary = animateColorAsState(t.primary, spec).value,
        onPrimary = animateColorAsState(t.onPrimary, spec).value,
        primaryContainer = animateColorAsState(t.primaryContainer, spec).value,
        onPrimaryContainer = animateColorAsState(t.onPrimaryContainer, spec).value,
        inversePrimary = animateColorAsState(t.inversePrimary, spec).value,
        secondary = animateColorAsState(t.secondary, spec).value,
        onSecondary = animateColorAsState(t.onSecondary, spec).value,
        secondaryContainer = animateColorAsState(t.secondaryContainer, spec).value,
        onSecondaryContainer = animateColorAsState(t.onSecondaryContainer, spec).value,
        tertiary = animateColorAsState(t.tertiary, spec).value,
        onTertiary = animateColorAsState(t.onTertiary, spec).value,
        tertiaryContainer = animateColorAsState(t.tertiaryContainer, spec).value,
        onTertiaryContainer = animateColorAsState(t.onTertiaryContainer, spec).value,
        background = animateColorAsState(t.background, spec).value,
        onBackground = animateColorAsState(t.onBackground, spec).value,
        surface = animateColorAsState(t.surface, spec).value,
        onSurface = animateColorAsState(t.onSurface, spec).value,
        surfaceVariant = animateColorAsState(t.surfaceVariant, spec).value,
        onSurfaceVariant = animateColorAsState(t.onSurfaceVariant, spec).value,
        surfaceTint = animateColorAsState(t.surfaceTint, spec).value,
        inverseSurface = animateColorAsState(t.inverseSurface, spec).value,
        inverseOnSurface = animateColorAsState(t.inverseOnSurface, spec).value,
        error = animateColorAsState(t.error, spec).value,
        onError = animateColorAsState(t.onError, spec).value,
        errorContainer = animateColorAsState(t.errorContainer, spec).value,
        onErrorContainer = animateColorAsState(t.onErrorContainer, spec).value,
        outline = animateColorAsState(t.outline, spec).value,
        outlineVariant = animateColorAsState(t.outlineVariant, spec).value,
        scrim = animateColorAsState(t.scrim, spec).value,
        surfaceBright = animateColorAsState(t.surfaceBright, spec).value,
        surfaceDim = animateColorAsState(t.surfaceDim, spec).value,
        surfaceContainer = animateColorAsState(t.surfaceContainer, spec).value,
        surfaceContainerHigh = animateColorAsState(t.surfaceContainerHigh, spec).value,
        surfaceContainerHighest = animateColorAsState(t.surfaceContainerHighest, spec).value,
        surfaceContainerLow = animateColorAsState(t.surfaceContainerLow, spec).value,
        surfaceContainerLowest = animateColorAsState(t.surfaceContainerLowest, spec).value,
        primaryFixed = animateColorAsState(t.primaryFixed, spec).value,
        primaryFixedDim = animateColorAsState(t.primaryFixedDim, spec).value,
        onPrimaryFixed = animateColorAsState(t.onPrimaryFixed, spec).value,
        onPrimaryFixedVariant = animateColorAsState(t.onPrimaryFixedVariant, spec).value,
        secondaryFixed = animateColorAsState(t.secondaryFixed, spec).value,
        secondaryFixedDim = animateColorAsState(t.secondaryFixedDim, spec).value,
        onSecondaryFixed = animateColorAsState(t.onSecondaryFixed, spec).value,
        onSecondaryFixedVariant = animateColorAsState(t.onSecondaryFixedVariant, spec).value,
        tertiaryFixed = animateColorAsState(t.tertiaryFixed, spec).value,
        tertiaryFixedDim = animateColorAsState(t.tertiaryFixedDim, spec).value,
        onTertiaryFixed = animateColorAsState(t.onTertiaryFixed, spec).value,
        onTertiaryFixedVariant = animateColorAsState(t.onTertiaryFixedVariant, spec).value,
    )
}

// A color of the wheel for item i (the list's avatars, the tasks' badges).
fun hueColor(i: Int): Color {
    val h = ((i * 47) % 360) / 60f
    val s = 0.45f
    val v = 0.72f
    val c = v * s
    val x = c * (1 - abs(h % 2f - 1))
    val m = v - c
    var r = 0f
    var g = 0f
    var b = 0f
    if (h < 1) { r = c; g = x }
    else if (h < 2) { r = x; g = c }
    else if (h < 3) { g = c; b = x }
    else if (h < 4) { g = x; b = c }
    else if (h < 5) { r = x; b = c }
    else { r = c; b = x }
    return Color(r + m, g + m, b + m, 1f)
}

// A small titled section of the components screen.
@Composable
fun Section(title: String, content: @Composable () -> Unit) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            content()
        }
    }
}

// On a phone, above a section that needs what phones seldom have (a hardware keyboard): a warning.
@Composable
fun DesktopNote(text: String) {
    if (!onAndroid) return
    val cs = MaterialTheme.colorScheme
    Row(modifier = Modifier.fillMaxWidth().background(cs.tertiaryContainer, RoundedCornerShape(12.dp)).padding(12.dp, 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.Filled.Info, contentDescription = "Uyarı", modifier = Modifier.size(18.dp), tint = cs.onTertiaryContainer)
        Text(text, modifier = Modifier.weight(1f), color = cs.onTertiaryContainer, style = MaterialTheme.typography.bodySmall)
    }
}
