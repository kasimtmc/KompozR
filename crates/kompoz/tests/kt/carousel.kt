// Material's carousels as Compose's samples write them (material3 1.5's CarouselSamples, as written; their
// @Sampled/@Preview annotations left out). Their Android resources — `R.drawable.carousel_image_N`,
// `R.string.carousel_image_N_description`, `painterResource(id = …)`, `stringResource(…)` — are KompozR's (Compose
// Multiplatform's): `R.*` is tests/composeResources' `Res` (its images 3:2 ones of a color of their own, its
// descriptions "Carousel image N", what the samples' own aren't here), so their local `data class CarouselItem`s'
// resource properties are CMP's DrawableResource and StringResource where Android's are `@DrawableRes Int`s.

// ---- CarouselSamples.kt ----

@Composable
fun HorizontalMultiBrowseCarouselSample() {

    data class CarouselItem(
        val id: Int,
        @DrawableRes val imageResId: DrawableResource,
        @StringRes val contentDescriptionResId: StringResource,
    )

    val items =
        listOf(
            CarouselItem(0, R.drawable.carousel_image_1, R.string.carousel_image_1_description),
            CarouselItem(1, R.drawable.carousel_image_2, R.string.carousel_image_2_description),
            CarouselItem(2, R.drawable.carousel_image_3, R.string.carousel_image_3_description),
            CarouselItem(3, R.drawable.carousel_image_4, R.string.carousel_image_4_description),
            CarouselItem(4, R.drawable.carousel_image_5, R.string.carousel_image_5_description),
        )

    val state = rememberCarouselState { items.count() }
    val animationScope = rememberCoroutineScope()
    HorizontalMultiBrowseCarousel(
        state = state,
        modifier = Modifier.fillMaxWidth().height(221.dp),
        preferredItemWidth = 186.dp,
        itemSpacing = 8.dp,
        contentPadding = PaddingValues(horizontal = 16.dp),
    ) { i ->
        val item = items[i]
        Image(
            modifier =
                Modifier.height(205.dp)
                    .fillMaxWidth()
                    .clickable(true, "Tap to focus", Role.Image) {
                        animationScope.launch { state.animateScrollToItem(i) }
                    }
                    .maskClip(MaterialTheme.shapes.extraLarge),
            painter = painterResource(id = item.imageResId),
            contentDescription = stringResource(item.contentDescriptionResId),
            contentScale = ContentScale.Crop,
        )
    }
}

@Composable
fun HorizontalUncontainedCarouselSample() {

    data class CarouselItem(
        val id: Int,
        @DrawableRes val imageResId: DrawableResource,
        @StringRes val contentDescriptionResId: StringResource,
    )

    val items =
        listOf(
            CarouselItem(0, R.drawable.carousel_image_1, R.string.carousel_image_1_description),
            CarouselItem(1, R.drawable.carousel_image_2, R.string.carousel_image_2_description),
            CarouselItem(2, R.drawable.carousel_image_3, R.string.carousel_image_3_description),
            CarouselItem(3, R.drawable.carousel_image_4, R.string.carousel_image_4_description),
            CarouselItem(4, R.drawable.carousel_image_5, R.string.carousel_image_5_description),
        )
    HorizontalUncontainedCarousel(
        state = rememberCarouselState { items.count() },
        modifier = Modifier.width(412.dp).height(221.dp),
        itemWidth = 186.dp,
        itemSpacing = 8.dp,
        contentPadding = PaddingValues(horizontal = 16.dp),
    ) { i ->
        val item = items[i]
        Image(
            modifier = Modifier.height(205.dp).maskClip(MaterialTheme.shapes.extraLarge),
            painter = painterResource(id = item.imageResId),
            contentDescription = stringResource(item.contentDescriptionResId),
            contentScale = ContentScale.Crop,
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HorizontalCenteredHeroCarouselSample() {

    data class CarouselItem(
        val id: Int,
        @DrawableRes val imageResId: DrawableResource,
        @StringRes val contentDescriptionResId: StringResource,
    )

    val items =
        listOf(
            CarouselItem(0, R.drawable.carousel_image_1, R.string.carousel_image_1_description),
            CarouselItem(1, R.drawable.carousel_image_2, R.string.carousel_image_2_description),
            CarouselItem(2, R.drawable.carousel_image_3, R.string.carousel_image_3_description),
            CarouselItem(3, R.drawable.carousel_image_4, R.string.carousel_image_4_description),
            CarouselItem(4, R.drawable.carousel_image_5, R.string.carousel_image_5_description),
        )
    val state = rememberCarouselState { items.count() }
    val animationScope = rememberCoroutineScope()
    HorizontalCenteredHeroCarousel(
        state = state,
        modifier = Modifier.fillMaxWidth().height(221.dp).padding(horizontal = 24.dp),
        itemSpacing = 8.dp,
        contentPadding = PaddingValues(horizontal = 16.dp),
    ) { i ->
        val item = items[i]
        Image(
            modifier =
                Modifier.fillMaxWidth()
                    .height(205.dp)
                    .maskClip(MaterialTheme.shapes.extraLarge)
                    .clickable(true, "Tap to focus", Role.Image) {
                        animationScope.launch { state.animateScrollToItem(i) }
                    },
            painter = painterResource(id = item.imageResId),
            contentDescription = stringResource(item.contentDescriptionResId),
            contentScale = ContentScale.Crop,
        )
    }
}

@Composable
fun FadingHorizontalMultiBrowseCarouselSample() {

    data class CarouselItem(
        val id: Int,
        @DrawableRes val imageResId: DrawableResource,
        @StringRes val contentDescriptionResId: StringResource,
    )

    val items =
        listOf(
            CarouselItem(0, R.drawable.carousel_image_1, R.string.carousel_image_1_description),
            CarouselItem(1, R.drawable.carousel_image_2, R.string.carousel_image_2_description),
            CarouselItem(2, R.drawable.carousel_image_3, R.string.carousel_image_3_description),
            CarouselItem(3, R.drawable.carousel_image_4, R.string.carousel_image_4_description),
            CarouselItem(4, R.drawable.carousel_image_5, R.string.carousel_image_5_description),
        )
    val state = rememberCarouselState { items.count() }
    HorizontalMultiBrowseCarousel(
        state = state,
        modifier = Modifier.width(412.dp).height(221.dp),
        preferredItemWidth = 186.dp,
        itemSpacing = 8.dp,
        contentPadding = PaddingValues(horizontal = 16.dp),
    ) { i ->
        val item = items[i]
        // For item 1 and 4, create a stacked item layout that clips two images independently
        // to the item's mask
        if (i == 1 || i == 4) {
            Column(
                modifier = Modifier.height(205.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Image(
                    modifier =
                        Modifier.fillMaxWidth()
                            .fillMaxHeight(.5f)
                            .maskClip(MaterialTheme.shapes.extraLarge)
                            .maskBorder(
                                BorderStroke(3.dp, Color.Magenta),
                                MaterialTheme.shapes.extraLarge,
                            ),
                    painter = painterResource(id = item.imageResId),
                    contentDescription = stringResource(item.contentDescriptionResId),
                    contentScale = ContentScale.Crop,
                )
                Image(
                    modifier =
                        Modifier.fillMaxSize()
                            .maskClip(RoundedCornerShape(8.dp))
                            .maskBorder(BorderStroke(5.dp, Color.Green), RoundedCornerShape(8.dp)),
                    painter = painterResource(id = item.imageResId),
                    contentDescription = stringResource(item.contentDescriptionResId),
                    contentScale = ContentScale.Crop,
                )
            }
        } else {
            // Mask using a generic path shape
            val pathShape = remember {
                object : Shape {
                    override fun createOutline(
                        size: Size,
                        layoutDirection: LayoutDirection,
                        density: Density,
                    ): Outline {
                        val roundRect =
                            RoundRect(0f, 0f, size.width, size.height, CornerRadius(30f))
                        val shapePath = Path().apply { addRoundRect(roundRect) }
                        return Outline.Generic(shapePath)
                    }
                }
            }
            Box(
                modifier =
                    Modifier.height(205.dp)
                        .maskClip(pathShape)
                        .maskBorder(BorderStroke(5.dp, Color.Red), pathShape)
            ) {
                Image(
                    painter = painterResource(id = item.imageResId),
                    contentDescription = stringResource(item.contentDescriptionResId),
                    modifier = Modifier.fillMaxSize(),
                    contentScale = ContentScale.Crop,
                )
                ElevatedAssistChip(
                    onClick = { /* Do something! */ },
                    label = { Text("Image $i") },
                    modifier =
                        Modifier.graphicsLayer {
                            // Fade the chip in once the carousel item's size is large enough to
                            // display the entire chip
                            alpha =
                                lerp(
                                    0f,
                                    1f,
                                    max(
                                        size.width - (carouselItemDrawInfo.maxSize) +
                                            carouselItemDrawInfo.size,
                                        0f,
                                    ) / size.width,
                                )
                            // Translate the chip to be pinned to the left side of the item's mask
                            translationX = carouselItemDrawInfo.maskRect.left + 8.dp.toPx()
                        },
                    leadingIcon = {
                        Icon(
                            Icons.Filled.Image,
                            contentDescription = "Localized description",
                            Modifier.size(AssistChipDefaults.IconSize),
                        )
                    },
                )
            }
        }
    }
}

@Composable
fun CarouselWithShowAllButtonSample() {

    data class CarouselItem(
        val id: Int,
        @DrawableRes val imageResId: DrawableResource,
        @StringRes val contentDescriptionResId: StringResource,
    )

    val items =
        listOf(
            CarouselItem(0, R.drawable.carousel_image_1, R.string.carousel_image_1_description),
            CarouselItem(1, R.drawable.carousel_image_2, R.string.carousel_image_2_description),
            CarouselItem(2, R.drawable.carousel_image_3, R.string.carousel_image_3_description),
            CarouselItem(3, R.drawable.carousel_image_4, R.string.carousel_image_4_description),
            CarouselItem(4, R.drawable.carousel_image_5, R.string.carousel_image_5_description),
        )
    var showAllItems by remember { mutableStateOf(false) }
    BackHandler(enabled = showAllItems) { showAllItems = false }
    if (showAllItems) {
        // Shows the grid page directly. For better user experience and navigation patterns,
        // consider navigating to a dedicated screen
        LazyVerticalGrid(
            columns = GridCells.Fixed(2),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier.fillMaxSize().padding(top = 8.dp),
        ) {
            items(items.size) { i ->
                val item = items[i]
                Image(
                    painter = painterResource(id = item.imageResId),
                    contentDescription = stringResource(item.contentDescriptionResId),
                    modifier = Modifier.height(205.dp).clip(MaterialTheme.shapes.extraLarge),
                    contentScale = ContentScale.Crop,
                )
            }
        }
    } else {
        Column(
            modifier =
                Modifier.fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(vertical = 16.dp)
        ) {
            HorizontalMultiBrowseCarousel(
                state = rememberCarouselState { items.count() },
                modifier = Modifier.width(412.dp).height(221.dp),
                preferredItemWidth = 186.dp,
                itemSpacing = 8.dp,
                contentPadding = PaddingValues(horizontal = 16.dp),
            ) { i ->
                val item = items[i]
                Image(
                    modifier = Modifier.height(205.dp).maskClip(MaterialTheme.shapes.extraLarge),
                    painter = painterResource(id = item.imageResId),
                    contentDescription = stringResource(item.contentDescriptionResId),
                    contentScale = ContentScale.Crop,
                )
            }
            TextButton(
                onClick = { showAllItems = true },
                modifier =
                    Modifier.align(Alignment.End).padding(vertical = 4.dp, horizontal = 16.dp),
            ) {
                Text("Show all")
            }
            // An example of the content for the scrollable page
            (0..4).forEach { index ->
                Spacer(modifier = Modifier.height(16.dp))
                Box(
                    modifier =
                        Modifier.fillMaxWidth()
                            .height(140.dp)
                            .padding(horizontal = 16.dp)
                            .background(
                                MaterialTheme.colorScheme.surfaceVariant,
                                RoundedCornerShape(16.dp),
                            )
                ) {}
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MultiAspectCarouselLazyRowSample() {

    data class CarouselItem(
        val id: Int,
        @DrawableRes val imageResId: DrawableResource,
        @StringRes val contentDescriptionResId: StringResource,
        val mainAxisSize: Dp,
    )

    val items =
        listOf(
            CarouselItem(
                0,
                R.drawable.carousel_image_1,
                R.string.carousel_image_1_description,
                305.dp,
            ),
            CarouselItem(
                1,
                R.drawable.carousel_image_2,
                R.string.carousel_image_2_description,
                205.dp,
            ),
            CarouselItem(
                2,
                R.drawable.carousel_image_3,
                R.string.carousel_image_3_description,
                275.dp,
            ),
            CarouselItem(
                3,
                R.drawable.carousel_image_4,
                R.string.carousel_image_4_description,
                350.dp,
            ),
            CarouselItem(
                4,
                R.drawable.carousel_image_5,
                R.string.carousel_image_5_description,
                100.dp,
            ),
        )

    MultiAspectCarouselScope {
        val state = rememberLazyListState()
        LazyRow(
            state = state,
            contentPadding = PaddingValues(16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier.fillMaxWidth().height(221.dp),
        ) {
            itemsIndexed(items) { i, item ->
                val drawInfo = remember { MultiAspectCarouselItemDrawInfo(i, state) }
                Image(
                    painter = painterResource(id = item.imageResId),
                    contentDescription = stringResource(item.contentDescriptionResId),
                    modifier =
                        Modifier.width(item.mainAxisSize)
                            .height(205.dp)
                            .maskClip(MaterialTheme.shapes.extraLarge, drawInfo),
                    contentScale = ContentScale.Crop,
                )
            }
        }
    }
}
