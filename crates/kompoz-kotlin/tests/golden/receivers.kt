// What a method call on a value calls is what the value's type has, the type inferred as Kotlin infers it: written,
// given by `remember { … }`'s block (its own locals too), a delegate's, a parameter's, a lambda's declared function
// type, a receiver lambda's (`with`, `run`), a call's result, a shared value's (`Rc<dyn Measurable>`). A method only one
// type has is that type's on a value of a type KompozR can't tell; one several have, an error asking for the type.
val LocalBase = compositionLocalOf { 1 }
val LocalTwice = compositionLocalOf { 0 }
val LocalHalf = compositionLocalWithComputedDefaultOf { LocalBase.currentValue / 2 }

@Composable
fun Animated(seekable: SeekableTransitionState<Int>, deferred: DeferredTransitionState<Int>) {
    val written: Animatable<Float, AnimationVector1D> = madeElsewhere()
    val remembered = remember { Animatable(0f) }
    val built = remember { val a = Animatable(0f); a }
    var delegated by remember { mutableStateOf(Animatable(0f)) }
    val unknown = madeElsewhere()
    LaunchedEffect(Unit) {
        written.animateTo(1f)
        remembered.animateTo(1f)
        built.animateTo(1f)
        delegated.animateTo(1f)
        seekable.animateTo(2)
        deferred.animateTo(3)
        with(remembered) { updateBounds(upperBound = 2f) }
        remembered.let { it }.animateTo(3f)
        listOf(remembered).first().animateTo(4f)
        remember { Animatable(0f) }.animateTo(4f)
        unknown.updateBounds(upperBound = 1f)
        val stop: suspend (Animatable<Float, AnimationVector1D>) -> Unit = { it.stop() }
        stop(remembered)
    }
}

@Composable
fun Unclear() {
    val unknown = madeElsewhere()
    LaunchedEffect(Unit) { unknown.animateTo(5f) }
}

@Composable
fun Computed(tint: State<Int>) {
    CompositionLocalProvider(LocalTwice providesComputed { LocalBase.currentValue * 2 + tint.value }, LocalBase provides 4) {
        Text("${LocalTwice.current} ${LocalHalf.current}")
    }
}
