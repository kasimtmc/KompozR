// A member extension called with both its receivers implicit: Painter's `fun DrawScope.draw(size, alpha = DefaultAlpha,
// colorFilter = null)` in `with(painter) { draw(…) }` inside a DrawScope lambda — the dispatch receiver `with`'s (a
// `VectorPainter`, Kotlin's subtype of the `Painter` interface it implements), the extension receiver the enclosing
// scope (the innermost DrawScope: `translate`'s in a `translate { }` block). The Rust method takes the scope first,
// lent again, the arguments reading it first. `colorFilter: ColorFilter?` is an `Option<&ColorFilter>`: a value lent, a
// nullable one as a reference, `null` as `None`. (`size / 2f` is a `Size`: a value's operator with a number keeps its
// type, as Kotlin's `Size.div(Float)`.)
@Composable
fun PaintedIcons(tint: Color, filter: ColorFilter?) {
    val painter = rememberVectorPainter(Icons.Filled.Favorite)
    Canvas(Modifier.size(48.dp)) {
        with(painter) {
            draw(size)
            draw(Size(24f, 24f), alpha = 0.5f, colorFilter = ColorFilter.tint(tint))
            draw(size, colorFilter = filter)
            draw(size, colorFilter = null)
        }
        translate(left = 8f) {
            with(painter) { draw(size / 2f, 1f, ColorFilter.tint(tint)) }
        }
    }
}

// The same in a receiver lambda of a declared function type, as SliderWithTrackIconsSample draws its track's icons
// (the scope a `ContentDrawScope`'s, a `DrawScope`).
@Composable
fun TrackIcons(color: Color) {
    val icon = rememberVectorPainter(Icons.Filled.Star)
    val drawAt: DrawScope.(Offset, Color) -> Unit = { offset, c ->
        translate(offset.x, offset.y) {
            with(icon) { draw(Size(20f, 20f), colorFilter = ColorFilter.tint(c)) }
        }
    }
    Box(Modifier.size(40.dp).drawWithContent {
        drawContent()
        drawAt(Offset(0f, 10f), color)
        with(icon) { draw(size, alpha = 0.5f) }
    })
}
