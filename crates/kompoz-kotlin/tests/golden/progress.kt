// Progress indicators as Compose writes them: a shared closure an API object property gives (its type a closure
// alias, `AmplitudeFunction` = `Rc<dyn Fn(f32) -> f32>`) fits a function parameter and is passed as it is; a receiver
// lambda's `this` lent mutably (`DrawScope.() -> Unit`, `&mut DrawScope`) given on to a `&mut DrawScope` parameter is
// reborrowed; overloads told apart by their first argument (a lambda or a `Float` progress, none: the endless one).

@Composable
fun Progress() {
    Column {
        CircularWavyProgressIndicator(
            progress = { 0.5f },
            amplitude = WavyProgressIndicatorDefaults.indicatorAmplitude,
        )
        LinearProgressIndicator(
            progress = { 0.5f },
            drawStopIndicator = {
                ProgressIndicatorDefaults.drawStopIndicator(
                    drawScope = this,
                    stopSize = 4.dp,
                    color = Color.Red,
                    strokeCap = StrokeCap.Butt,
                )
            },
        )
        LinearProgressIndicator(progress = 0.25f)
        LinearProgressIndicator()
        LinearWavyProgressIndicator(progress = { 0.5f }, amplitude = { 0.5f }, waveSpeed = 0.dp)
    }
}
