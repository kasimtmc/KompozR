// Material's slots and objects as Compose writes them: a nullable slot given an `if` of a lambda and `null`
// (`leadingIcon = if (selected) { { Icon(…) } } else null`), or of two lambdas, an `else if` too; a slot lambda
// written last for a parameter before the last (`label = { … }`, the rest left to their defaults) named for the macro;
// an object's function's result typing an overload (`Modifier.size(IconButtonDefaults.mediumContainerSize(…))`, a
// DpSize); a value of a type of its own to a shared trait object's parameter (`Icon(rememberVectorPainter(…))`, a
// Painter); Kotlin's `Random(seed)` and `shuffle(random)`.
@Composable
fun Chips(selected: Boolean, wide: Boolean) {
    val tasks = remember { mutableStateListOf(1, 2, 3) }
    Column {
        FilterChip(
            selected = selected,
            onClick = { tasks.shuffle(Random(7)) },
            label = { Text("Filtre") },
            leadingIcon = if (selected) { { Icon(Icons.Filled.Done, contentDescription = null) } } else { null },
        )
        FilterChip(
            selected = selected,
            onClick = {},
            label = { Text("İkisi") },
            leadingIcon = if (selected) { { Icon(Icons.Filled.Done, contentDescription = null) } } else if (wide) { { Icon(Icons.Filled.Home, contentDescription = null) } } else null,
        )
        InputChip(selected = selected, onClick = {}, label = { Text("Giriş") })
        IconButton(onClick = {}, modifier = Modifier.size(IconButtonDefaults.mediumContainerSize(IconButtonDefaults.IconButtonWidthOption.Wide))) {
            Icon(rememberVectorPainter(image = Icons.Filled.Lock), contentDescription = null, tint = Color.Red)
        }
    }
}
