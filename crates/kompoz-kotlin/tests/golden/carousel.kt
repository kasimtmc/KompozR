// Carousels: an item lambda `{ i -> }` on its CarouselItemScope receiver (its member extensions `Modifier.maskClip`
// on the modifier, `carouselItemDrawInfo` a property of the receiver, `8.dp.toPx()` in a graphics layer — a Density),
// a plain function's receiver lambda typed (`GenericShape { size, _ -> }`: Path), Path's defaulted direction, and the
// multi-aspect scope reached from lambdas nested in it.

@Composable
fun Browse(count: Int) {
    val state = rememberCarouselState { count }
    HorizontalMultiBrowseCarousel(state = state, preferredItemWidth = 186.dp, itemSpacing = 8.dp) { i ->
        val shape = remember {
            GenericShape { size, _ ->
                addRoundRect(RoundRect(0f, 0f, size.width, size.height, CornerRadius(30f)))
            }
        }
        Box(
            Modifier.fillMaxSize()
                .maskClip(shape)
                .maskBorder(BorderStroke(1.dp, Color.Red), MaterialTheme.shapes.small)
                .graphicsLayer { translationX = carouselItemDrawInfo.maskRect.left + 8.dp.toPx() }
        ) {
            Text("$i")
        }
    }
}

@Composable
fun Aspects(widths: List<Dp>) {
    MultiAspectCarouselScope {
        val state = rememberLazyListState()
        LazyRow(state = state) {
            itemsIndexed(widths) { i, width ->
                val drawInfo = remember { MultiAspectCarouselItemDrawInfo(i, state) }
                Box(Modifier.width(width).maskClip(RectangleShape, drawInfo))
            }
        }
    }
}
