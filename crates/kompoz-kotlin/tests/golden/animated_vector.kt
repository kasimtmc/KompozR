@Composable
fun AnimatedVectorSample(xml: String) {
    val image = remember(xml) { AnimatedImageVector.fromAnimatedVectorXml(xml).getOrThrow() }
    var atEnd by remember { mutableStateOf(false) }
    Image(
        painter = rememberAnimatedVectorPainter(animatedImageVector = image, atEnd = atEnd),
        contentDescription = "Your content description",
        modifier = Modifier.size(64.dp).clickable { atEnd = !atEnd },
    )
}

@Composable
fun ReadOrNot(xml: String, seen: (Boolean, Int) -> Unit) {
    val read = AnimatedImageVector.fromAnimatedVectorXml("<animated-vector>$xml</animated-vector>")
    seen(read.isFailure, read.getOrNull()?.totalDuration ?: 0)
}
