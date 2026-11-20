// Foundation's styles, as Compose writes them: a fun interface's SAM constructor beside functions of its name taking
// other values (`Style { … }`, `Style(a, b)`), a parameter of a fun interface's type given a lambda (`style = { … }`),
// its value the interface's object, a `val` of one (`Rc<dyn Style>`); the members of a scope's supertraits
// (`StyleScope` → `SizeScope`, `StyleStateScope`, … → `Density`) and of their extension traits; a type's constructor
// alone (`MutableInteractionSource()`, `MutableStyleState(source)`), its generic result (`StyleStateKey(false)`);
// a parameter named as a member, a member call (`shape(shape)`); Kotlin's operators `get` and `set` of a type taking
// a key (`state[key] = value`, `state[key]`).
@Composable
fun Container(enabled: Boolean, shape: Shape, style: Style = Style) {
    val interactionSource = remember { MutableInteractionSource() }
    val styleState = remember(interactionSource) { MutableStyleState(interactionSource) }
    Box(
        Modifier.styleable(styleState) {
            shape(shape)
            background(Color.White)
            size(10.dp)
            translationX(2.dp.toPx())
            focused { animate(tween(100)) { background(Color.Blue) } }
            if (state[PlayingKey]) alpha(0.5f)
        }.styleable(null, style)
    )
}

val PlayingKey = StyleStateKey(false)

val Base = Style { background(Color.Red) }

@Composable
fun Uses(playing: Boolean) {
    val state = rememberUpdatedStyleState(null) { it[PlayingKey] = playing }
    Container(enabled = true, shape = RectangleShape, style = { alpha(0.5f) })
    Box(Modifier.styleable(state, Style(Base, Style { size(4.dp) })))
    Box(Modifier.styleable(state, Base then Style))
}
