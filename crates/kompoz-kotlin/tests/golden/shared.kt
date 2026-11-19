// Shared elements as Compose writes them: SharedTransitionLayout's receiver, its member extensions on a Modifier
// (`Modifier.….sharedElement(rememberSharedContentState(key = …), animatedVisibilityScope = this@AnimatedContent)`),
// a scope handed down and reached by `with(sharedTransitionScope) { }`, nested types of the scope
// (`SharedTransitionScope.ResizeMode.RemeasureToBounds`).

@Composable
fun Photo(open: Boolean) {
    SharedTransitionLayout(modifier = Modifier.fillMaxWidth()) {
        AnimatedContent(targetState = open, label = "photo") { o ->
            Box(modifier = Modifier.size(if (o) 80.dp else 40.dp).testTag(if (o) "büyük" else "küçük")
                    .sharedElement(rememberSharedContentState(key = "photo"), animatedVisibilityScope = this@AnimatedContent))
        }
    }
}

@Composable
fun Item(id: Int, sharedTransitionScope: SharedTransitionScope, animatedVisibilityScope: AnimatedVisibilityScope) {
    with(sharedTransitionScope) {
        Box(
            modifier = Modifier
                .sharedBounds(rememberSharedContentState(key = "öğe $id"), animatedVisibilityScope, resizeMode = SharedTransitionScope.ResizeMode.RemeasureToBounds)
                .size(40.dp)
        )
    }
}
