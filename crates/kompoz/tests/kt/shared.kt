// Shared elements in a .kt file (kompoz_file!): SharedTransitionScope { modifier -> } laying itself out, a shared
// content state with a configuration, isTransitionActive and isMatchFound read in composition, skipToLookaheadSize()
// after a shared element, animateBounds in the scope it is (this@SharedTransitionScope).

@Composable
fun SharedFile(open: Boolean, log: (String) -> Unit) {
    SharedTransitionScope { modifier ->
        Column(modifier = modifier.fillMaxSize()) {
            AnimatedContent(targetState = open, transitionSpec = { fadeIn() togetherWith fadeOut() }) { o ->
                val photo = rememberSharedContentState(key = "photo", config = SharedContentConfig(permitTransformDuringDeferredTransition = false))
                log((if (o) "açık" else "dinlenme") + " $isTransitionActive ${photo.isMatchFound}")
                Box(
                    modifier = Modifier
                        .sharedElement(photo, this@AnimatedContent)
                        .size(if (o) 300.dp else 60.dp, 60.dp)
                        .skipToLookaheadSize()
                ) {
                    Box(modifier = Modifier.fillMaxSize().background(Color.Magenta).testTag(if (o) "atlayan iç" else "atlayan dış"))
                }
            }
            Box(modifier = Modifier.fillMaxWidth().height(100.dp), contentAlignment = if (open) Alignment.TopEnd else Alignment.TopStart) {
                Box(modifier = Modifier.animateBounds(this@SharedTransitionScope).size(200.dp, 40.dp).testTag("kayan"))
            }
        }
    }
}
